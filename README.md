# Ficflow

Ficflow is a software designed to help you track your fanfiction reading list and organize them the way you want.

![Ficflow GUI](assets/screenshots/gui_preview.png)

## Why Ficflow?

I felt like there was a lack of ways to manage my AO3 reading list (other than having hundreds of tabs opened on my phone or saving the links in my notes), so I decided to work on a solution! It's also my first time making a project in Rust, so if you have any criticism about my approach, feel free to open an issue and teach me!

## Features (Planned, the order is approximate)

- [x] Refactor to make sure both the CLI and the future GUI can work at the same time
- [ ] Application version in the settings doesn't match the github one
- [ ] Create a feature-complete GUI version compatible with all OS (target for v1)
- [ ] Bump deps and move to edition 2024 or 2025
- [ ] Ability to change column order by grabbing them
- [ ] Ability to choose the text size / zoom level in the settings
- [ ] New feature of auto-shelves (based on ships, fandoms, relationship,...)
- [ ] Possibility to choose the location of the library (defaults to appdata/ ~/.ficflow like now), with persistence of that config
- [ ] Release the software on the AUR (can we make both the CLI and GUI work at the same time?)
- [ ] Import/Export database to create backups
- [ ] Ability to login to access restricted fics
- [ ] Create a rule that automatically checks for updates when the GUI is open and sends notifications to the user in case there is a new one (settings, refresh every x time, + notifications for all manual and auto refreshes when a new update was found! And an inbox too)
- [ ] add works with not only single fics, but collections, a user list of fics, a tag, a user profile,...
- [ ] Add a search system based on tags, with an option to filter out fics already catalogued
- [ ] Setting to make it start in the background on computer start-up, and stay on background when closing (opt-in, different settings or same? SHould add a proper, cleaner "quit" option if so)
- [ ] Create a mobile phone app (target for v2)

## Usage

### Available Commands

```
# Basic operations
add <fic_id_or_url>     Add a fanfiction (accepts AO3 URLs or IDs)
get <fic_id>            Show fanfiction details
delete <fic_id>         Remove a fanfiction
list                    List all saved fanfictions
wipe                    Delete all saved fanfictions

# Update commands
chapter <fic_id> <num>  Update last chapter read
status <fic_id> <status> Update reading status (inprogress, read, plantoread, paused, abandoned)
reads <fic_id> <count>  Update read count
rating <fic_id> <rating> Set rating (1-5, one-five, or none/clear to remove)
note <fic_id> [text]    Add/update/remove personal note (omit text to remove)

# Shelves (custom groupings of fanfictions)
shelf create <name>               Create a new shelf
shelf delete <shelf_id>           Delete a shelf
shelf list                        List all shelves
shelf add <fic_id> <shelf_id>     Add a fanfiction to a shelf
shelf remove <fic_id> <shelf_id>  Remove a fanfiction from a shelf
shelf show <shelf_id>             List the fanfictions in a shelf
```

For more details, run `ficflow` with `--help`.

## Contributing

Any feedback or contributions are welcome! If you have ideas, suggestions, or improvements, feel free to open an issue or submit a pull request.

