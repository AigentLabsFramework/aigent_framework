import discord
from discord import app_commands, ButtonStyle
from discord.ui import Button, View, Modal, TextInput

# Configuration
TOKEN = "YOUR_DISCORD_BOT_TOKEN"  # Replace with your bot token
categories = ["Cars", "Tools", "Digital Goods"]
listings = {}  # {category: [items]}
item_counter = 0

# Set up bot with intents
intents = discord.Intents.default()
intents.message_content = True
client = discord.Client(intents=intents)
tree = app_commands.CommandTree(client)

# Start command: Show main menu
@tree.command(name="start", description="Start Aigent Rental")
async def start(interaction: discord.Interaction):
    view = View()
    view.add_item(Button(label="Items for Rent", custom_id="items_for_rent", style=ButtonStyle.primary))
    view.add_item(Button(label="Search Items", custom_id="search_items", style=ButtonStyle.primary))
    view.add_item(Button(label="List Your Item", custom_id="list_item", style=ButtonStyle.primary))
    view.add_item(Button(label="My Rentals", custom_id="my_rentals", style=ButtonStyle.primary))
    await interaction.response.send_message("Welcome to Aigent Rental!", view=view, ephemeral=True)

# Modal for listing items
class ListingModal(Modal, title="List Your Item"):
    category = TextInput(label="Category", placeholder="e.g., Cars")
    item_type = TextInput(label="Type", placeholder="physical or digital")
    location = TextInput(label="Location (if physical)", required=False)
    service_name = TextInput(label="Service Name", placeholder="e.g., Car Rent in Melbourne")
    price = TextInput(label="Price with duration", placeholder="e.g., 50 SOL per 1d")
    deposit = TextInput(label="Deposit", placeholder="e.g., 100 SOL")
    description = TextInput(label="Description", placeholder="Short description")
    links = TextInput(label="Links (optional)", required=False)

    async def on_submit(self, interaction: discord.Interaction):
        global item_counter
        item_counter += 1
        listing = {
            'id': item_counter,
            'category': self.category.value,
            'type': self.item_type.value.lower(),
            'location': self.location.value if self.location.value else None,
            'service_name': self.service_name.value,
            'price': self.price.value,
            'deposit': float(self.deposit.value.split()[0]),
            'description': self.description.value,
            'links': self.links.value,
            'owner': interaction.user.id
        }
        listings.setdefault(listing['category'], []).append(listing)
        await interaction.response.send_message(f"Listed: {listing['service_name']}", ephemeral=True)

# Modal for searching items
class SearchModal(Modal, title="Search Items"):
    keyword = TextInput(label="Keyword", placeholder="e.g., car")
    location = TextInput(label="Location (optional)", placeholder="e.g., Melbourne", required=False)

    async def on_submit(self, interaction: discord.Interaction):
        keyword = self.keyword.value.lower()
        location = self.location.value.lower() if self.location.value else None
        matches = []
        for cat in listings.values():
            for item in cat:
                if keyword in item['service_name'].lower() or keyword in item['description'].lower():
                    if item['type'] == 'digital' or (item['type'] == 'physical' and (location is None or location in item['location'].lower())):
                        matches.append(item)
        if matches:
            view = View()
            for item in matches:
                view.add_item(Button(label=f"{item['service_name']} - {item['price']}", custom_id=f"rent_{item['id']}", style=ButtonStyle.success))
            await interaction.response.send_message(f"Found {len(matches)} items:", view=view, ephemeral=True)
        else:
            await interaction.response.send_message("No items found.", ephemeral=True)

# Interaction handler for buttons
@client.event
async def on_interaction(interaction: discord.Interaction):
    if interaction.type == discord.InteractionType.component:
        custom_id = interaction.data['custom_id']
        if custom_id == 'list_item':
            await interaction.response.send_modal(ListingModal())
        elif custom_id == 'search_items':
            await interaction.response.send_modal(SearchModal())
        elif custom_id.startswith('rent_'):
            item_id = int(custom_id[5:])
            for cat in listings.values():
                for item in cat:
                    if item['id'] == item_id:
                        await interaction.response.send_message(
                            f"Rent: {item['service_name']}\n"
                            f"Price: {item['price']}\n"
                            f"Deposit: {item['deposit']} SOL\n"
                            "Send payment to start escrow (integration TBD).",
                            ephemeral=True
                        )
                        return
            await interaction.response.send_message("Item not found.", ephemeral=True)

# Bot startup
@client.event
async def on_ready():
    print(f'Logged in as {client.user}')
    await tree.sync()

client.run(TOKEN)