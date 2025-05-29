# Ficflow

Ficflow is a software designed to help you track your fanfiction reading list and organize them the way you want.

## Why Ficflow?

I felt like there was a lack of ways to manage my AO3 reading list (other than having hundreds of tabs opened on my phone or saving the links in my notes), so I decided to work on a solution! It's also my first time making a project in Rust, so if you have any criticism about my approach, feel free to open an issue and teach me!

## Features (Planned, the order is approximate)

- [x] Finalise a first version of Ficflow available on CLI only
    - [x] Add a way to list your saved fanfics
    - [x] Allow users to edit the custom fields (rating, status, chapters, reads, notes)
    - [x] Support for direct AO3 URLs when adding fanfictions
- [ ] Implement a function that checks for fic updates
- [ ] Allow users to create shelves and organise more flexibly
- [x] Refactor to make sure both the CLI and the future GUI can work at the same time
- [ ] Create a feature-complete GUI version compatible with all OS (target for v1)
- [ ] Import/Export database to create backups
- [ ] Ability to login to access restricted fics
- [ ] Create a rule that automatically checks for updates when the GUI is open and sends notifications to the user in case there is a new one
- [ ] Add a search system based on tags, with an option to filter out fics already catalogued
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
```

For more details, run `ficflow` with `--help`.

## Contributing

Any feedback or contributions are welcome! If you have ideas, suggestions, or improvements, feel free to open an issue or submit a pull request.

