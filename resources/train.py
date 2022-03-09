import os
from typing import Any

import numpy
import random
import torch
import torchvision
from glob import glob
from PIL import Image
from tqdm import tqdm

ENCODER_INPUT_WIDTH = 255
ENCODER_INPUT_HEIGHT = 255
LATENT_SPACE_SIZE = 8
LEARNING_RATE = 1e-4
EPOCHS = 100
BATCH_SIZE = 16  # Remember that we get this^2 tensor when computing.

DEVICE = torch.device("cuda")
#device = torch.device("cuda:0" if torch.cuda.is_available() else "cpu")

# Define our mutations to perform on the network.
# We expect a PIL as an Input and return a Tensor
corruptions = torchvision.transforms.Compose([
	torchvision.transforms.RandomVerticalFlip(),
	torchvision.transforms.RandomHorizontalFlip(),
	torchvision.transforms.RandomInvert(),
	torchvision.transforms.RandomRotation(45),
	torchvision.transforms.ColorJitter(),
	torchvision.transforms.Resize(int(ENCODER_INPUT_WIDTH*1.2)),  # 44 pixels of play to reshuffle.
	#torchvision.transforms.CenterCrop
	torchvision.transforms.RandomResizedCrop((ENCODER_INPUT_WIDTH, ENCODER_INPUT_HEIGHT)),  # This might be backwards.
	torchvision.transforms.ToTensor(),
])


# Define our model.
def build_model(latent_space: int):
	model = torch.nn.Sequential(
		torch.nn.Conv2d(in_channels=3, out_channels=64, kernel_size=3, stride=1, padding=1),
		torch.nn.Conv2d(in_channels=64, out_channels=128, kernel_size=3, stride=1, padding=1),
		torch.nn.LeakyReLU(inplace=True),
		torch.nn.AvgPool2d(3),
		torch.nn.Conv2d(in_channels=128, out_channels=256, kernel_size=3, stride=1, padding=1),
		torch.nn.Conv2d(in_channels=256, out_channels=512, kernel_size=3, stride=1, padding=1),
		torch.nn.LeakyReLU(inplace=True),
		torch.nn.AvgPool2d(3),
		torch.nn.Conv2d(in_channels=512, out_channels=1024, kernel_size=3, stride=1, padding=1),
		torch.nn.Conv2d(in_channels=1024, out_channels=128, kernel_size=3, stride=1, padding=1),
		torch.nn.LeakyReLU(inplace=True),
		torch.nn.AvgPool2d(3),
		torch.nn.Flatten(),
		torch.nn.Linear(in_features=10368, out_features=1024),
		torch.nn.Linear(in_features=1024, out_features=latent_space)
	)
	return model


class PlainImageLoader(torchvision.datasets.VisionDataset):
	def __init__(self, root):
		super(PlainImageLoader, self).__init__(root)
		self.all_image_filenames = glob(os.path.join(root, "*.jpg"))
		self.all_image_filenames.extend(glob(os.path.join(root, "*.png")))

	def __getitem__(self, index: int) -> Any:

		return Image.open(self.all_image_filenames[index])

	def __len__(self) -> int:
		return len(self.all_image_filenames)


# Set up
def train(training_data_directory, model=None):
	if model is None:
		model = build_model(10).to(DEVICE)

	# Brace for run...
	loss_fn = torch.nn.CosineEmbeddingLoss()
	optimizer = torch.optim.Adam(model.parameters(), lr=LEARNING_RATE)
	#dataset = torchvision.datasets.ImageFolder(training_data_directory, transform=corruptions)
	dataset = PlainImageLoader(training_data_directory)  # We will do the corruptions ourselves.
	dataset_loader = torch.utils.data.DataLoader(dataset, batch_size=BATCH_SIZE, shuffle=True, num_workers=4, pin_memory=True)

	# Training loop:
	for epoch_idx in range(EPOCHS):
		dataloop = tqdm(dataset_loader)
		total_epoch_loss = 0.0
		for batch_idx, data in enumerate(dataloop):
			data = data.to(device=DEVICE)
			corrupted_data = corruptions(data)
			optimizer.zero_grad()

			# Maybe shuffle pairings.
			labels = torch.Tensor([1] * BATCH_SIZE)  # All entries start paired.
			for idx in range(BATCH_SIZE-1):
				if random.choice([True, False]):  # 50/50 shot of swapping this with another entry.
					# Don't want to re-swap something that was already swapped.
					for _ in range(3):  # 3 retries
						swap_index = random.randint(idx+1, BATCH_SIZE)
						if labels[swap_index] == 1:  # Warning: this is lazy and dumb.
							break
					# Swap this pair.
					temp = corrupted_data[idx, :, :, :]
					corrupted_data[idx, :, :, :] = corrupted_data[swap_index, :, :, :]
					corrupted_data[swap_index, :, :, :] = temp
					labels[idx] = -1
					labels[swap_index] = -1
			labels = labels.to(device=DEVICE)

			# Forward
			left = model(data)
			right = model(corrupted_data)

			# Embedding pairs are 1 if they're the same and -1 if they're not.
			# We match up embeddings based on their classes.
			loss = loss_fn(left, right, labels)

			# Backward
			loss.backward()
			optimizer.step()

			# Log status.
			total_epoch_loss += loss.item()

		print(f"Total epoch loss: {total_epoch_loss}")
		torch.save(model.state_dict(), f"checkpoints/checkpoint_{epoch_idx}")
	torch.save(model, "result_model.pt")


def finalize(encoder):
	# Build final model:
	device = torch.device("cpu")
	encoder_cpu = encoder.to(device)
	example = torch.rand(1, 3, ENCODER_INPUT_HEIGHT, ENCODER_INPUT_WIDTH).to(device)
	torch.onnx.export(encoder_cpu, example, "encoder_cpu.onnx", example_outputs=encoder_cpu(example), opset_version=11)
	traced_script_module = torch.jit.trace(encoder_cpu, example)
	traced_script_module.save("encoder_cpu.pt")


def main():
	model = build_model(LATENT_SPACE_SIZE)
	train("/home/joseph/Pictures", model)
	finalize(model)

if __name__ == "__main__":
	main()