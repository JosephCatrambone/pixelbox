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
LATENT_SPACE_SIZE = 64
LEARNING_RATE = 1e-6
EPOCHS = 100
BATCH_SIZE = 4

DEVICE = torch.device("cuda")
#device = torch.device("cuda:0" if torch.cuda.is_available() else "cpu")

# Define our mutations to perform on the network.
# We expect a PIL as an Input and return a Tensor
corruptions = torchvision.transforms.Compose([
	torchvision.transforms.RandomVerticalFlip(),
	torchvision.transforms.RandomHorizontalFlip(),
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
		torch.nn.Conv2d(in_channels=256, out_channels=256, kernel_size=3, stride=1, padding=1),
		torch.nn.LeakyReLU(inplace=True),
		torch.nn.AvgPool2d(3),
		torch.nn.Conv2d(in_channels=512, out_channels=1024, kernel_size=3, stride=1, padding=1),
		torch.nn.Conv2d(in_channels=1024, out_channels=128, kernel_size=3, stride=1, padding=1),  # Bottleneck!
		torch.nn.LeakyReLU(inplace=True),
		torch.nn.AvgPool2d(3),
		torch.nn.Flatten(),
		torch.nn.Linear(in_features=10368, out_features=1024),
		torch.nn.Linear(in_features=1024, out_features=latent_space),
		torch.nn.Tanh()
	)
	print("Built model:")
	print(model)
	return model


class PlainImageLoader(torchvision.datasets.VisionDataset):
	def __init__(self, root):
		super(PlainImageLoader, self).__init__(root)
		self.all_image_filenames = glob(os.path.join(root, "*.jpg"))
		self.all_image_filenames.extend(glob(os.path.join(root, "**", "*.jpg")))  # Include subdirectories.
		self.all_image_filenames.extend(glob(os.path.join(root, "*.png")))
		self.all_image_filenames.extend(glob(os.path.join(root, "**", "*.png")))
		# Filter images that can't get loaded.  This is a little slow but saves some headache.
		to_remove = list()
		for filename in self.all_image_filenames:
			try:
				_ = Image.open(filename).convert("RGB")
			except KeyboardInterrupt:
				raise
			except Exception as e:
				print(f"Failed to read {filename}: {e}")
				to_remove.append(filename)
		for filename in to_remove:
			self.all_image_filenames.remove(filename)

	def __getitem__(self, index: int) -> Any:
		img_left = corruptions(Image.open(self.all_image_filenames[index]).convert("RGB"))
		if random.choice([True, False]):
			other_index = random.randint(0, len(self.all_image_filenames)-1)
			img_right = corruptions(Image.open(self.all_image_filenames[other_index]).convert("RGB"))
			label = -1.0
			if other_index == index:
				label = 1.0  # In the slim chance we happen to pick exactly the same index at random...
		else:
			img_right = corruptions(Image.open(self.all_image_filenames[index]).convert("RGB"))
			label = 1.0
		label = torch.tensor(label)
		return img_left, img_right, label

	def __len__(self) -> int:
		return len(self.all_image_filenames)


# Set up
def train(training_data_directory, model=None):
	if model is None:
		model = build_model(10)

	model = model.to(DEVICE)

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
		for batch_idx, (data_left, data_right, labels) in enumerate(dataloop):
			data_left = data_left.to(device=DEVICE)
			data_right = data_right.to(device=DEVICE)
			labels = labels.to(device=DEVICE)
			optimizer.zero_grad()

			# Forward
			left = model(data_left)
			right = model(data_right)

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
	train("E:\\Dropbox\\Photos", model)
	finalize(model)

if __name__ == "__main__":
	main()