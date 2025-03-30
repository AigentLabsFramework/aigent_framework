import discord
from discord import app_commands, ButtonStyle
from discord.ui import Button, View
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from anchorpy import Program, Provider, Wallet
import json
from datetime import datetime

# Configuration
ADMIN_ID = 123456789  # Your Discord ID
ADMIN_WALLET = Pubkey.from_string("YourAdminWalletPubkeyHere")
PROGRAM_ID = Pubkey.from_string("HNjDWKszkwv9NmQKVrzfC42Qg6R78C1EUtvHLtfJTASx")
AGENT_KEYPAIR = Keypair.from_seed(bytes([1] * 32))  # Replace securely

# Storage
categories = ["Electronics", "Clothing"]
listings = {}
user_trades = {}
item_id_counter = 0
admin_fee_percent = 0.75

# Solana setup
provider = Provider.local("https://api.devnet.solana.com")
with open("path/to/aigent_framework.json") as f:
    idl = json.load(f)
program = Program(idl, PROGRAM_ID, provider, Wallet(AGENT_KEYPAIR))

intents = discord.Intents.default()
intents.message_content = True
client = discord.Client(intents=intents)
tree = app_commands.CommandTree(client)

def calculate_admin_fee(price):
    return (price * admin_fee_percent) / 100

@tree.command(name="start", description="Start the marketplace bot")
async def start(interaction: discord.Interaction):
    view = View()
    view.add_item(Button(label="Items for Sale", custom_id="items_for_sale", style=ButtonStyle.primary))
    view.add_item(Button(label="Search Items", custom_id="search_items", style=ButtonStyle.primary))
    view.add_item(Button(label="Sell Your Item", custom_id="sell_item", style=ButtonStyle.primary))
    view.add_item(Button(label="My History", custom_id="my_history", style=ButtonStyle.primary))
    if interaction.user.id == ADMIN_ID:
        view.add_item(Button(label="Admin Control", custom_id="admin_control", style=ButtonStyle.danger))
    await interaction.response.send_message("Welcome to the Marketplace!", view=view)

@tree.command(name="admin_control", description="Admin controls")
async def admin_control(interaction: discord.Interaction):
    if interaction.user.id != ADMIN_ID:
        await interaction.response.send_message("Unauthorized!", ephemeral=True)
        return
    view = View()
    view.add_item(Button(label="Add Category", custom_id="add_category", style=ButtonStyle.primary))
    view.add_item(Button(label="Remove Category", custom_id="remove_category", style=ButtonStyle.primary))
    view.add_item(Button(label="Set Fee", custom_id="set_fee", style=ButtonStyle.primary))
    view.add_item(Button(label="Change Wallet", custom_id="change_wallet", style=ButtonStyle.primary))
    view.add_item(Button(label="View Stats", custom_id="view_stats", style=ButtonStyle.primary))
    await interaction.response.send_message("Admin Control Panel:", view=view)

@client.event
async def on_interaction(interaction: discord.Interaction):
    if not interaction.type == discord.InteractionType.component:
        return
    custom_id = interaction.data['custom_id']
    user_id = interaction.user.id

    if custom_id == 'items_for_sale':
        view = View()
        for cat in categories:
            view.add_item(Button(label=cat, custom_id=f'cat_{cat}'))
        await interaction.response.edit_message(content="Select a category:", view=view)

    elif custom_id.startswith('cat_'):
        cat = custom_id[4:]
        if cat in listings and listings[cat]:
            view = View()
            for item in listings[cat]:
                view.add_item(Button(label=f"{item['desc']} - {item['price']} SOL", custom_id=f'buy_{item["id"]}'))
            await interaction.response.edit_message(content=f"Items in {cat}:", view=view)
        else:
            await interaction.response.edit_message(content="No items in this category.")

    elif custom_id == 'search_items':
        await interaction.response.send_modal(SearchModal())

    elif custom_id == 'sell_item':
        view = View()
        for cat in categories:
            view.add_item(Button(label=cat, custom_id=f'sell_cat_{cat}'))
        await interaction.response.edit_message(content="Select category:", view=view)

    elif custom_id.startswith('sell_cat_'):
        category = custom_id[9:]
        await interaction.response.send_modal(SellModal(category))

    elif custom_id == 'my_history':
        view = View()
        view.add_item(Button(label="All Trades", custom_id="all_trades"))
        view.add_item(Button(label="Active Trades", custom_id="active_trades"))
        await interaction.response.edit_message(content="Your History:", view=view)

    elif custom_id == 'all_trades':
        trades = user_trades.get(user_id, [])
        text = "\n".join(f"TX {t['tx_id']}: {t['amount']} SOL at {t['time']}" for t in trades) or "No trades."
        await interaction.response.edit_message(content=f"All Trades:\n{text}", view=None)

    elif custom_id == 'active_trades':
        trades = user_trades.get(user_id, [])
        if trades:
            view = View()
            for t in trades:
                view.add_item(Button(label=f"Confirm {t['tx_id']}", custom_id=f'confirm_{t["tx_id"]}'))
            text = "\n".join(f"TX {t['tx_id']}: {t['amount']} SOL" for t in trades)
            await interaction.response.edit_message(content=f"Active Trades:\n{text}", view=view)
        else:
            await interaction.response.edit_message(content="No active trades.", view=None)

    elif custom_id.startswith('buy_'):
        item_id = int(custom_id[4:])
        for cat in listings:
            for item in listings[cat]:
                if item['id'] == item_id:
                    tx_id = Keypair().pubkey()
                    admin_fee = calculate_admin_fee(item['price'])
                    msg = (f"To buy {item['desc']}:\n"
                           f"1. Send {admin_fee} SOL to {ADMIN_WALLET}\n"
                           f"2. Call start_escrow with:\n   - tx_id: {tx_id}\n   - rent: {item['price']}\n   - deposit: 0\n   - release_secs: 604800\n   - token_mint: None")
                    await interaction.response.edit_message(content=msg, view=None)
                    user_trades.setdefault(user_id, []).append({
                        'tx_id': tx_id, 'item_id': item_id, 'amount': item['price'], 'time': datetime.now()
                    })
                    break

    elif custom_id.startswith('confirm_'):
        tx_id = Pubkey.from_string(custom_id[8:])
        await interaction.response.edit_message(content=f"Call confirm_receipt with tx_id={tx_id} using your wallet.", view=None)
        user_trades[user_id] = [t for t in user_trades.get(user_id, []) if str(t['tx_id']) != str(tx_id)]

    # Admin Controls
    elif custom_id == 'add_category' and user_id == ADMIN_ID:
        await interaction.response.send_modal(AddCategoryModal())

    elif custom_id == 'remove_category' and user_id == ADMIN_ID:
        view = View()
        for cat in categories:
            view.add_item(Button(label=cat, custom_id=f'remove_cat_{cat}'))
        await interaction.response.edit_message(content="Select category to remove:", view=view)

    elif custom_id.startswith('remove_cat_') and user_id == ADMIN_ID:
        cat = custom_id[10:]
        categories.remove(cat)
        listings.pop(cat, None)
        await interaction.response.edit_message(content=f"{cat} removed.", view=None)

    elif custom_id == 'set_fee' and user_id == ADMIN_ID:
        await interaction.response.send_modal(SetFeeModal())

    elif custom_id == 'change_wallet' and user_id == ADMIN_ID:
        await interaction.response.send_modal(ChangeWalletModal())

    elif custom_id == 'view_stats' and user_id == ADMIN_ID:
        total_traded = sum(sum(t['amount'] for t in trades) for trades in user_trades.values())
        total_users = len(user_trades)
        await interaction.response.edit_message(content=f"Total Traded: {total_traded} SOL\nTotal Users: {total_users}", view=None)

class SearchModal(discord.ui.Modal, title="Search Items"):
    keyword = discord.ui.TextInput(label="Keyword", placeholder="Enter search term")
    async def on_submit(self, interaction: discord.Interaction):
        matches = [item for cat in listings.values() for item in cat if self.keyword.value.lower() in item['desc'].lower()]
        view = View()
        for item in matches:
            view.add_item(Button(label=f"{item['desc']} - {item['price']} SOL", custom_id=f'buy_{item["id"]}'))
        await interaction.response.edit_message(content=f"Found {len(matches)} items:", view=view if matches else None)

class SellModal(discord.ui.Modal, title="List an Item"):
    def __init__(self, category):
        super().__init__()
        self.category = category
        self.desc = discord.ui.TextInput(label="Description", placeholder="Item description")
        self.price = discord.ui.TextInput(label="Price (SOL)", placeholder="e.g., 10.5")
        self.link = discord.ui.TextInput(label="Link (optional)", required=False)
    async def on_submit(self, interaction: discord.Interaction):
        global item_id_counter
        item_id_counter += 1
        price = float(self.price.value)
        listings.setdefault(self.category, []).append({
            'id': item_id_counter, 'desc': self.desc.value, 'price': price, 'link': self.link.value or '', 'seller': interaction.user.id
        })
        await interaction.response.edit_message(content=f"Listed: {self.desc.value} for {price} SOL", view=None)

class AddCategoryModal(discord.ui.Modal, title="Add Category"):
    name = discord.ui.TextInput(label="Category Name", placeholder="e.g., Books")
    async def on_submit(self, interaction: discord.Interaction):
        if self.name.value not in categories:
            categories.append(self.name.value)
            await interaction.response.edit_message(content=f"Added: {self.name.value}", view=None)
        else:
            await interaction.response.edit_message(content="Category exists.", view=None)

class SetFeeModal(discord.ui.Modal, title="Set Admin Fee"):
    fee = discord.ui.TextInput(label="Fee Percentage", placeholder="e.g., 0.75")
    async def on_submit(self, interaction: discord.Interaction):
        global admin_fee_percent
        admin_fee_percent = float(self.fee.value)
        await interaction.response.edit_message(content=f"Fee set to {admin_fee_percent}%", view=None)

class ChangeWalletModal(discord.ui.Modal, title="Change Admin Wallet"):
    wallet = discord.ui.TextInput(label="Wallet Address", placeholder="Solana address")
    async def on_submit(self, interaction: discord.Interaction):
        global ADMIN_WALLET
        ADMIN_WALLET = Pubkey.from_string(self.wallet.value)
        await interaction.response.edit_message(content="Wallet updated.", view=None)

@client.event
async def on_ready():
    await tree.sync()
    print(f'Logged in as {client.user}')

client.run("YOUR_DISCORD_BOT_TOKEN")