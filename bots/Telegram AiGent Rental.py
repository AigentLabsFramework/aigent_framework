from telegram import InlineKeyboardButton, InlineKeyboardMarkup
from telegram.ext import Updater, CommandHandler, CallbackQueryHandler, MessageHandler, Filters

# Configuration
TOKEN = "YOUR_TELEGRAM_BOT_TOKEN"  # Replace with your bot token
ADMIN_ID = 123456789  # Replace with your Telegram user ID for admin access
categories = ["Cars", "Tools", "Digital Goods"]
listings = {}  # {category: [items]}
item_counter = 0

# Start command: Show main menu
def start(update, context):
    keyboard = [
        [InlineKeyboardButton("Items for Rent", callback_data='items_for_rent')],
        [InlineKeyboardButton("Search Items", callback_data='search_items')],
        [InlineKeyboardButton("List Your Item", callback_data='list_item')],
        [InlineKeyboardButton("My Rentals", callback_data='my_rentals')]
    ]
    if update.message.from_user.id == ADMIN_ID:
        keyboard.append([InlineKeyboardButton("Admin Control", callback_data='admin_control')])
    update.message.reply_text('Welcome to Aigent Rental!', reply_markup=InlineKeyboardMarkup(keyboard))

# Button handler: Handle navigation
def button(update, context):
    query = update.callback_query
    query.answer()
    user_id = query.from_user.id
    data = query.data

    if data == 'list_item':
        # Show category selection
        keyboard = [[InlineKeyboardButton(cat, callback_data=f'select_cat_{cat}')] for cat in categories]
        query.edit_message_text('Select category:', reply_markup=InlineKeyboardMarkup(keyboard))

    elif data.startswith('select_cat_'):
        # Store selected category and ask for item type
        cat = data[11:]
        context.user_data['listing'] = {'category': cat}
        keyboard = [
            [InlineKeyboardButton("Physical", callback_data='type_physical')],
            [InlineKeyboardButton("Digital", callback_data='type_digital')]
        ]
        query.edit_message_text('Is the item physical or digital?', reply_markup=InlineKeyboardMarkup(keyboard))

    elif data.startswith('type_'):
        # Store item type and proceed
        item_type = data[5:]
        context.user_data['listing']['type'] = item_type
        if item_type == 'physical':
            query.edit_message_text('Enter location (e.g., Melbourne):')
            context.user_data['state'] = 'enter_location'
        else:
            query.edit_message_text('Enter service name (e.g., Graphic Design Template):')
            context.user_data['state'] = 'enter_service_name'

    elif data == 'search_items':
        query.edit_message_text('Enter a keyword to search:')
        context.user_data['state'] = 'enter_search_keyword'

    elif data.startswith('rent_'):
        item_id = int(data[5:])
        # Find the item
        for cat in listings.values():
            for item in cat:
                if item['id'] == item_id:
                    query.edit_message_text(
                        f"Rent: {item['service_name']}\n"
                        f"Price: {item['price']}\n"
                        f"Deposit: {item['deposit']} SOL\n"
                        "Send payment to start escrow (integration TBD)."
                    )
                    return
        query.edit_message_text("Item not found.")

# Text handler: Process user inputs
def handle_text(update, context):
    user_id = update.message.from_user.id
    text = update.message.text
    state = context.user_data.get('state', '')

    if state == 'enter_location':
        context.user_data['listing']['location'] = text
        update.message.reply_text('Enter service name (e.g., Car Rent in Melbourne):')
        context.user_data['state'] = 'enter_service_name'

    elif state == 'enter_service_name':
        context.user_data['listing']['service_name'] = text
        update.message.reply_text('Enter price with duration (e.g., 50 SOL per 1d):')
        context.user_data['state'] = 'enter_price'

    elif state == 'enter_price':
        context.user_data['listing']['price'] = text
        update.message.reply_text('Enter deposit amount (e.g., 100 SOL):')
        context.user_data['state'] = 'enter_deposit'

    elif state == 'enter_deposit':
        try:
            deposit = float(text.split()[0])
            context.user_data['listing']['deposit'] = deposit
            update.message.reply_text('Enter short description:')
            context.user_data['state'] = 'enter_description'
        except ValueError:
            update.message.reply_text('Invalid deposit. Enter a number (e.g., 100 SOL).')

    elif state == 'enter_description':
        context.user_data['listing']['description'] = text
        update.message.reply_text('Enter optional links (comma-separated):')
        context.user_data['state'] = 'enter_links'

    elif state == 'enter_links':
        context.user_data['listing']['links'] = text
        listing = context.user_data['listing']
        cat = listing['category']
        global item_counter
        item_counter += 1
        listing['id'] = item_counter
        listing['owner'] = user_id
        listings.setdefault(cat, []).append(listing)
        update.message.reply_text(f'Listed: {listing["service_name"]}')
        context.user_data['state'] = ''
        del context.user_data['listing']

    elif state == 'enter_search_keyword':
        context.user_data['search_keyword'] = text.lower()
        update.message.reply_text('Enter location (or "none" to skip):')
        context.user_data['state'] = 'enter_search_location'

    elif state == 'enter_search_location':
        location = text.lower() if text.lower() != 'none' else None
        keyword = context.user_data['search_keyword']
        matches = []
        for cat in listings.values():
            for item in cat:
                if keyword in item['service_name'].lower() or keyword in item['description'].lower():
                    if item['type'] == 'digital' or (item['type'] == 'physical' and (location is None or location in item['location'].lower())):
                        matches.append(item)
        if matches:
            keyboard = [[InlineKeyboardButton(f"{item['service_name']} - {item['price']}", callback_data=f'rent_{item["id"]}')] for item in matches]
            update.message.reply_text(f'Found {len(matches)} items:', reply_markup=InlineKeyboardMarkup(keyboard))
        else:
            update.message.reply_text('No items found.')
        context.user_data['state'] = ''
        del context.user_data['search_keyword']

# Main function to run the bot
def main():
    updater = Updater(TOKEN, use_context=True)
    dp = updater.dispatcher
    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CallbackQueryHandler(button))
    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text))
    updater.start_polling()
    updater.idle()

if __name__ == '__main__':
    main()