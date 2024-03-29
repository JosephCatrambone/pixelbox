import json
import os
from collections import namedtuple
from datetime import datetime
from typing import Any, Type

import numpy
import random
import torch
import torchvision
import torchvision.transforms.v2
from glob import glob
from PIL import Image
from tqdm import tqdm

try:
	import wandb
except Exception as e:
	import time
	print(f"Failed to load WANDB: {e}")
	time.sleep(2)
	wandb = None

Configuration = namedtuple("Configuration", "ENCODER_INPUT_WIDTH ENCODER_INPUT_HEIGHT LATENT_SPACE_SIZE LEARNING_RATE EPOCHS BATCH_SIZE DATA_PATH ARCHITECTURE NOTES TRAINING_LOSSES")

DEVICE = torch.device("cuda")
#device = torch.device("cuda:0" if torch.cuda.is_available() else "cpu")

# Define our model.
def build_model(latent_space: int):
	enet = torchvision.models.efficientnet_b0()
	for name, module in enet.named_modules():
		if name == "features":
			features = module
	assert features is not None
	poolfn = torch.nn.AdaptiveAvgPool2d(1)
	model = torch.nn.Sequential(
		features,
		poolfn,
		torch.nn.Flatten(1,),
		torch.nn.Linear(in_features=1280, out_features=latent_space),
		torch.nn.Tanh(),
	)
	print("Built model:")
	print(model)
	return model


class PlainImageLoader(torchvision.datasets.VisionDataset):
	def __init__(self, root, corruptions, scan_for_broken_images:bool = False):
		super(PlainImageLoader, self).__init__(root)
		self.corruptions = corruptions
		self.all_image_filenames = glob(os.path.join(root, "*.jpg"))
		self.all_image_filenames.extend(glob(os.path.join(root, "**", "*.jpg")))  # Include subdirectories.
		self.all_image_filenames.extend(glob(os.path.join(root, "*.png")))
		self.all_image_filenames.extend(glob(os.path.join(root, "**", "*.png")))
		self.all_image_filenames.extend(glob(os.path.join(root, "*.gif")))
		self.all_image_filenames.extend(glob(os.path.join(root, "**", "*.gif")))
		if scan_for_broken_images:
			# Filter images that can't get loaded.  This is a little slow but saves some headache.
			print("Scanning for broken images...")
			to_remove = list()
			for filename in tqdm(self.all_image_filenames):
				try:
					_ = Image.open(filename).convert("RGB")
				except KeyboardInterrupt:
					raise
				except Exception as e:
					print(f"Failed to read {filename}: {e}")
					to_remove.append(filename)
			for filename in to_remove:
				self.all_image_filenames.remove(filename)
			print(f"Training set has {len(self.all_image_filenames)} images.")

	def __getitem__(self, index: int) -> Any:
		img_left = self.corruptions(Image.open(self.all_image_filenames[index]).convert("RGB"))
		if random.choice([True, False]):
			other_index = random.randint(0, len(self.all_image_filenames)-1)
			img_right = self.corruptions(Image.open(self.all_image_filenames[other_index]).convert("RGB"))
			label = -1.0
			if other_index == index:
				label = 1.0  # In the slim chance we happen to pick exactly the same index at random...
		else:
			img_right = self.corruptions(Image.open(self.all_image_filenames[index]).convert("RGB"))
			label = 1.0
		label = torch.tensor(label)
		return img_left, img_right, label

	def __len__(self) -> int:
		return len(self.all_image_filenames)


# Set up
def train(model, config: Type[Configuration]):
	model = model.to(DEVICE)

	training_data_directory = config.DATA_PATH

	# Define our mutations to perform on the network.
	# We expect a PIL as an Input and return a Tensor
	corruptions = torchvision.transforms.Compose([
		#torchvision.transforms.RandomVerticalFlip(),
		#torchvision.transforms.RandomHorizontalFlip(),
		torchvision.transforms.RandomRotation(25),
		torchvision.transforms.ColorJitter(),
		# This line, coupled with random resized crops, means we have more scale variations in our images.
		torchvision.transforms.v2.RandomResize(int(config.ENCODER_INPUT_WIDTH*1.2), int(config.ENCODER_INPUT_WIDTH*1.4)),
		#torchvision.transforms.Resize(int(config.ENCODER_INPUT_WIDTH*1.2)),  # 44 pixels of play to reshuffle.
		#torchvision.transforms.CenterCrop
		torchvision.transforms.RandomResizedCrop((config.ENCODER_INPUT_WIDTH, config.ENCODER_INPUT_HEIGHT)),  # This might be backwards.
		torchvision.transforms.v2.RandomGrayscale(0.01),
		torchvision.transforms.v2.RandomInvert(0.001),
		torchvision.transforms.v2.GaussianBlur(5),
		torchvision.transforms.ToTensor(),
	])

	# Brace for run...
	loss_fn = torch.nn.CosineEmbeddingLoss()
	optimizer = torch.optim.Adam(model.parameters(), lr=config.LEARNING_RATE)
	#dataset = torchvision.datasets.ImageFolder(training_data_directory, transform=corruptions)
	dataset = PlainImageLoader(training_data_directory, corruptions)  # We will do the corruptions ourselves.
	dataset_loader = torch.utils.data.DataLoader(dataset, batch_size=config.BATCH_SIZE, shuffle=True, num_workers=4, pin_memory=True)

	# Report run start:
	if wandb:
		wandb.init(
			project="pixelbox-embedding-mk1",
			config=config._asdict()
		)

	# Training loop:
	epoch_losses = list()
	for epoch_idx in range(config.EPOCHS):
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
			if wandb:
				wandb.log({"loss": loss.item()})

		print(f"Epoch [{epoch_idx}/{config.EPOCHS}] loss: {total_epoch_loss}")
		epoch_losses.append(total_epoch_loss)
		#torch.save(model.state_dict(), f"checkpoints/checkpoint_{epoch_idx}")
		torch.save(model, f"checkpoints/checkpoint_{epoch_idx}.pt")
	torch.save(model, "result_model.pt")
	return epoch_losses


def finalize(encoder, config):
	# Build final model:
	device = torch.device("cpu")
	encoder_cpu = encoder.to(device)
	example = torch.rand(1, 3, config.ENCODER_INPUT_HEIGHT, config.ENCODER_INPUT_WIDTH).to(device)
	torch.onnx.export(encoder_cpu, example, "image_similarity.onnx", opset_version=11, do_constant_folding=True, input_names=['input',], output_names=['output',], dynamic_axes={'input': {0: 'batch_size'}, 'output':{0: 'batch_size'}})
	traced_script_module = torch.jit.trace(encoder_cpu, example)
	traced_script_module.save("image_similarity.pt")


def main():
	latent_space = 8
	model = build_model(latent_space)
	config = Configuration(
		ENCODER_INPUT_WIDTH=224,
		ENCODER_INPUT_HEIGHT=224,
		LATENT_SPACE_SIZE=latent_space,
		LEARNING_RATE=1e-4,
		EPOCHS=10,
		BATCH_SIZE=32,
		DATA_PATH="/home/joseph/MLData/train_512/",
		ARCHITECTURE=str(model),
		NOTES="""More image augmentations. Haven't done triplet comparison yet, so we're still in contrastive land.""",
		TRAINING_LOSSES=[],
	)
	log_timestamp = datetime.strftime(datetime.now(), "%Y%m%d%H%M%S")
	config.TRAINING_LOSSES.extend(
		train(model, config)
	)
	finalize(model, config)
	with open(f"./experiment_log_{log_timestamp}.txt", 'wt') as fout:
		json.dump(config._asdict(), fout)

if __name__ == "__main__":
	main()
