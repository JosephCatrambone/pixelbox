import numpy
import random
import torch
import torchvision
from tqdm import tqdm

ENCODER_INPUT_WIDTH = 255
ENCODER_INPUT_HEIGHT = 255

device = torch.device("cuda:0")
#device = torch.device("cuda:0" if torch.cuda.is_available() else "cpu")

# Define our mutations to perform on the network.
training_preprocessing = torchvision.transforms.Compose([
	torchvision.transforms.Resize(300),  # 44 pixels of play to reshuffle.
	#torchvision.transforms.CenterCrop
	torchvision.transforms.RandomResizedCrop((ENCODER_INPUT_WIDTH, ENCODER_INPUT_HEIGHT)),  # This might be backwards.
	torchvision.transforms.ToTensor(),
])

release_preprocessing = torchvision.transforms.Compose([
	torchvision.transforms.Resize((ENCODER_INPUT_WIDTH, ENCODER_INPUT_HEIGHT)),
	torchvision.transforms.ToTensor(),
])

corruptions = torch.nn.Sequential(
	torchvision.transforms.ColorJitter(),
	torchvision.transforms.RandomHorizontalFlip(),
	torchvision.transforms.RandomVerticalFlip(),
	#torchvision.transforms.RandomGrayscale(),
	#torchvision.transforms.ToTensor(),
	#torchvision.transforms.Normalize((0.5, 0.5, 0.5), (0.5, 0.5, 0.5))
)
#scripted_transforms = torch.jit.script(corruptions)

# Load data.
BATCH_SIZE = 4
#data_dir = './data/'
data_dir = "/home/joseph/Pictures/"
image_dataset = torchvision.datasets.ImageFolder(data_dir, training_preprocessing)
dataloader = torch.utils.data.DataLoader(image_dataset, batch_size=BATCH_SIZE, shuffle=True, num_workers=4)
dataset_size = len(image_dataset)
class_names = image_dataset.classes

# Define our model.
encoder = ImageEncoder()

class TwinNetwork(torch.nn.Module):
	def __init__(self, encoder):
		super().__init__()
		self.encoder = encoder
		self.distance = torch.nn.CosineSimilarity(dim=1, eps=1e-6)

	def forward(self, x_1, x_2):
		x_1 = self.encoder(x_1)
		x_2 = self.encoder(x_2)
		return self.distance(x_1, x_2)
		#return torch.abs(x_1 - x_2).sum()

net = TwinNetwork(encoder).to(device)

# Train the fuck out of it.
loss_fn = torch.nn.MSELoss() # Not BCELoss.
optimizer = torch.optim.SGD(net.parameters(), lr=0.01, momentum=0.7)

losses = list()

for epoch in tqdm(range(500)):
	batch_losses = list()
	for batch in dataloader:
		inputs, _labels = batch
		if inputs.shape[0] < 4:
			continue
		inputs = inputs.to(device)
		# We ignore labels and, for some examples, swap two of the images to mark them as false.
		x1 = corruptions(inputs)
		x2 = corruptions(inputs) # torch.zeros_like(x1)

		# Mutate x2 and swap some to make the negative case.
		# When we swap two entries, mark them as zero, 'cause they don't match.
		indices = torch.LongTensor([idx for idx in range(x2.shape[0])])
		indices[0] = 1
		indices[1] = 0
		x2[indices] = x2
		y = numpy.ones(x2.shape[0], dtype=numpy.float32)
		y[0] = 0
		y[1] = 0
		y = torch.Tensor(y).to(device)

		optimizer.zero_grad()
		out = net(x1, x2)
		loss = loss_fn(out, y)
		loss.backward()
		optimizer.step()
		batch_losses.append(loss.item())
		#writer.add_scalar('Loss/train', loss.item(), n_iter)
	print("Max/Min/Avg")
	print(max(batch_losses), end="\t\t")
	print(min(batch_losses), end="\t\t")
	print(sum(batch_losses)/len(batch_losses), end="\t\t")
	losses.append(sum(batch_losses)/len(batch_losses))
	if epoch % 10 == 0:
		torch.save(encoder.state_dict(), f"enc_epoch_{epoch}.pt")
# Build final model:
device = torch.device("cpu")
encoder_cpu = encoder.to(device)
example = torch.rand(1, 3, ImageEncoder.INPUT_SIZE, ImageEncoder.INPUT_SIZE).to(device)
torch.onnx.export(encoder_cpu, example, "encoder_cpu.onnx", example_outputs=encoder_cpu(example), opset_version=11)
traced_script_module = torch.jit.trace(encoder_cpu, example)
traced_script_module.save("encoder_cpu.pt")
