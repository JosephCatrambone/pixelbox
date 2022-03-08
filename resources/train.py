import numpy
import random
import torch
import torchvision
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


# Set up
def train(training_data_directory, model=None):
	if model is None:
		model = build_model(10).to(DEVICE)

	# Brace for run...
	loss_fn = torch.nn.CosineEmbeddingLoss()
	optimizer = torch.optim.Adam(model.parameters(), lr=LEARNING_RATE)
	dataset = torchvision.datasets.ImageFolder(training_data_directory, transform=corruptions)
	dataset_loader = torch.utils.data.DataLoader(dataset, batch_size=BATCH_SIZE, shuffle=True, num_workers=4, pin_memory=True)

	# Training loop:
	for epoch_idx in range(EPOCHS):
		dataloop = tqdm(dataset_loader)
		total_epoch_loss = 0.0
		for batch_idx, (data, targets) in enumerate(dataloop):
			data = data.to(device=DEVICE)
			optimizer.zero_grad()

			# Forward
			embeddings = model(data)

			# One embedding gives us n*(n-1) pairs of datapoints.
			# We rely on the batch being shuffled and having some of each class, but if the entire batch is unlucky
			# and we have all one class, it will be okay.
			# left takes [1, 2, 3, 4] and goes to [1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4]
			# right takes [1, 2, 3, 4] and goes to [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]
			left = torch.repeat_interleave(embeddings, embeddings.shape[0], axis=0)
			right = embeddings.repeat(embeddings.shape[0], 1)
			truth = list()
			for label_left in targets:
				for label_right in targets:
					truth.append(1.0 if label_left == label_right else -1.0)
			truth = torch.tensor(truth).to(DEVICE)

			# Embedding pairs are 1 if they're the same and -1 if they're not.
			# We match up embeddings based on their classes.
			loss = loss_fn(left, right, truth)

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