from telegram import Update, InlineKeyboardButton, InlineKeyboardMarkup
from telegram.ext import Application, CommandHandler, CallbackQueryHandler, MessageHandler, Filters
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from anchorpy import Program, Provider, Wallet
import json
from datetime import datetime

# Configuration
ADMIN_ID = 123456789  # Replace with your Telegram ID
ADMIN_WALLET = Pubkey.from_string("YourAdminWalletPubkeyHere")
PROGRAM_ID = Pubkey.from_string("HNjDWKszkwv9NmQKVrzfC42Qg6R78C1EUtvHLtfJTASx")
AGENT_KEYPAIR = Keypair.from_seed(bytes([1] * 32))  # Replace with actual agent keypair securely

# In-memory storage
categories = ["Electronics", "Clothing"]
listings = {}  # {category: [{id, desc, price, link, seller}]}
user_trades = {}  # {user_id: [{tx_id, item_id, amount, time}]}
item_id_counter = 0
admin_fee_percent = 0.75  # Additional fee %

# Solana client setup (simplified; configure with your RPC endpoint)
provider = Provider.local("https://api.devnet.solana.com")  # Use your RPC URL
with open("path/to/aigent_framework.json") as f:  # Your IDL file
    idl = json.load(f)
program = Program(idl, PROGRAM_ID, provider, Wallet(AGENT_KEYPAIR))

def calculate_admin_fee(price):
    return (price * admin_fee_percent) / 100

def start(update: Update, context):
    keyboard = [
        [InlineKeyboardButton("Items for Sale", callback_data='items_for_sale')],
        [InlineKeyboardButton("Search Items", callback_data='search_items')],
        [InlineKeyboardButton("Sell Your Item", callback_data='sell_item')],
        [InlineKeyboardButton("My History", callback_data='my_history')]
    ]
    if update.message.from_user.id == ADMIN_ID:
        keyboard.append([InlineKeyboardButton("Admin Control", callback_data='admin_control')])
    update.message.reply_text('Welcome to the Marketplace!', reply_markup=InlineKeyboardMarkup(keyboard))

def button(update: Update, context):
    query = update.callback_query
    query.answer()
    user_id = query.from_user.id
    data = query.data

    if data == 'items_for_sale':
        keyboard = [[InlineKeyboardButton(cat, callback_data=f'cat_{cat}')] for cat in categories]
        query.edit_message_text('Select a category:', reply_markup=InlineKeyboardMarkup(keyboard))

    elif data.startswith('cat_'):
        cat = data[4:]
        if cat in listings and listings[cat]:
            keyboard = [[InlineKeyboardButton(f"{item['desc']} - {item['price']} SOL", callback_data=f'buy_{item["id"]}')]
                        for item in listings[cat]]
            query.edit_message_text(f'Items in {cat}:', reply_markup=InlineKeyboardMarkup(keyboard))
        else:
            query.edit_message_text('No items in this category.')

    elif data == 'search_items':
        query.edit_message_text('Enter a keyword to search:')
        context.user_data['state'] = 'search_items'

    elif data == 'sell_item':
        keyboard = [[InlineKeyboardButton(cat, callback_data=f'sell_cat_{cat}')] for cat in categories]
        query.edit_message_text('Select category for your item:', reply_markup=InlineKeyboardMarkup(keyboard))

    elif data.startswith('sell_cat_'):
        context.user_data['sell_category'] = data[9:]
        query.edit_message_text('Enter item details (description, price, link) separated by commas:')
        context.user_data['state'] = 'sell_item'

    elif data == 'my_history':
        keyboard = [
            [InlineKeyboardButton("All Trades", callback_data='all_trades')],
            [InlineKeyboardButton("Active Trades", callback_data='active_trades')]
        ]
        query.edit_message_text('Your History:', reply_markup=InlineKeyboardMarkup(keyboard))

    elif data == 'all_trades':
        trades = user_trades.get(user_id, [])
        text = "\n".join(f"TX {t['tx_id']}: {t['amount']} SOL at {t['time']}" for t in trades) or "No trade history."
        query.edit_message_text(f"All Trades:\n{text}")

    elif data == 'active_trades':
        trades = user_trades.get(user_id, [])
        if trades:
            keyboard = [[InlineKeyboardButton(f"Confirm {t['tx_id']}", callback_data=f'confirm_{t["tx_id"]}')] for t in trades]
            text = "\n".join(f"TX {t['tx_id']}: {t['amount']} SOL" for t in trades)
            query.edit_message_text(f"Active Trades:\n{text}", reply_markup=InlineKeyboardMarkup(keyboard))
        else:
            query.edit_message_text("No active trades.")

    elif data.startswith('buy_'):
        item_id = int(data[4:])
        for cat in listings:
            for item in listings[cat]:
                if item['id'] == item_id:
                    tx_id = Keypair().pubkey()
                    admin_fee = calculate_admin_fee(item['price'])
                    msg = (f"To buy {item['desc']} for {item['price']} SOL:\n"
                           f"1. Send {admin_fee} SOL to {ADMIN_WALLET}\n"
                           f"2. Use your wallet to call start_escrow with:\n"
                           f"   - tx_id: {tx_id}\n   - rent: {item['price']}\n   - deposit: 0\n   - release_secs: 604800\n   - token_mint: None")
                    query.edit_message_text(msg)
                    user_trades.setdefault(user_id, []).append({
                        'tx_id': tx_id, 'item_id': item_id, 'amount': item['price'], 'time': datetime.now()
                    })
                    break

    elif data.startswith('confirm_'):
        tx_id = Pubkey.from_string(data[8:])
        # In practice, buyer would sign this via wallet; here, bot instructs user
        query.edit_message_text(f"Call confirm_receipt with tx_id={tx_id} using your wallet to release funds.")
        # Remove from active trades after confirmation (simplified)
        user_trades[user_id] = [t for t in user_trades.get(user_id, []) if str(t['tx_id']) != str(tx_id)]

    # Admin Controls
    elif data == 'admin_control' and user_id == ADMIN_ID:
        keyboard = [
            [InlineKeyboardButton("Add Category", callback_data='add_category')],
            [InlineKeyboardButton("Remove Category", callback_data='remove_category')],
            [InlineKeyboardButton("Set Fee", callback_data='set_fee')],
            [InlineKeyboardButton("Change Wallet", callback_data='change_wallet')],
            [InlineKeyboardButton("View Stats", callback_data='view_stats')]
        ]
        query.edit_message_text('Admin Control Panel:', reply_markup=InlineKeyboardMarkup(keyboard))

    elif data == 'add_category':
        query.edit_message_text('Enter new category name:')
        context.user_data['state'] = 'add_category'

    elif data == 'remove_category':
        keyboard = [[InlineKeyboardButton(cat, callback_data=f'remove_cat_{cat}')] for cat in categories]
        query.edit_message_text('Select category to remove:', reply_markup=InlineKeyboardMarkup(keyboard))

    elif data.startswith('remove_cat_'):
        cat = data[10:]
        categories.remove(cat)
        listings.pop(cat, None)
        query.edit_message_text(f"{cat} removed.")

    elif data == 'set_fee':
        query.edit_message_text('Enter new fee percentage (e.g., 0.75):')
        context.user_data['state'] = 'set_fee'

    elif data == 'change_wallet':
        query.edit_message_text('Enter new admin wallet address:')
        context.user_data['state'] = 'change_wallet'

    elif data == 'view_stats':
        total_traded = sum(sum(t['amount'] for t in trades) for trades in user_trades.values())
        total_users = len(user_trades)
        query.edit_message_text(f"Total Traded: {total_traded} SOL\nTotal Users: {total_users}")

def text_handler(update: Update, context):
    user_id = update.message.from_user.id
    text = update.message.text
    state = context.user_data.get('state', '')

    if state == 'search_items':
        matches = [item for cat in listings.values() for item in cat if text.lower() in item['desc'].lower()]
        keyboard = [[InlineKeyboardButton(f"{item['desc']} - {item['price']} SOL", callback_data=f'buy_{item["id"]}')]
                    for item in matches] if matches else []
        update.message.reply_text(f'Found {len(matches)} items:' if matches else 'No items found.',
                                  reply_markup=InlineKeyboardMarkup(keyboard) if matches else None)
        context.user_data['state'] = ''

    elif state == 'sell_item':
        try:
            parts = text.split(',', 2)
            desc, price = parts[0], float(parts[1].strip())
            link = parts[2].strip() if len(parts) > 2 else ''
            cat = context.user_data['sell_category']
            global item_id_counter
            item_id_counter += 1
            listings.setdefault(cat, []).append({'id': item_id_counter, 'desc': desc, 'price': price, 'link': link, 'seller': user_id})
            update.message.reply_text(f'Listed: {desc} for {price} SOL')
        except:
            update.message.reply_text('Format: description, price, link (link optional)')
        context.user_data['state'] = ''

    elif state == 'add_category' and user_id == ADMIN_ID:
        if text not in categories:
            categories.append(text)
            update.message.reply_text(f'Added: {text}')
        else:
            update.message.reply_text('Category exists.')
        context.user_data['state'] = ''

    elif state == 'set_fee' and user_id == ADMIN_ID:
        try:
            global admin_fee_percent
            admin_fee_percent = float(text)
            update.message.reply_text(f'Fee set to {admin_fee_percent}%')
        except:
            update.message.reply_text('Invalid number.')
        context.user_data['state'] = ''

    elif state == 'change_wallet' and user_id == ADMIN_ID:
        try:
            global ADMIN_WALLET
            ADMIN_WALLET = Pubkey.from_string(text)
            update.message.reply_text(f'Wallet updated.')
        except:
            update.message.reply_text('Invalid address.')
        context.user_data['state'] = ''

def main():
    application = Application.builder().token("YOUR_TELEGRAM_BOT_TOKEN").build()
    application.add_handler(CommandHandler("start", start))
    application.add_handler(CallbackQueryHandler(button))
    application.add_handler(MessageHandler(Filters.text & ~Filters.command, text_handler))
    application.run_polling()

if __name__ == '__main__':
    main()