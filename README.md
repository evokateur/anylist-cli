# AnyList CLI

a CLI client for [AnyList](https://www.anylist.com), built on [anylist_rs](https://github.com/phildenhoff/anylist_rs).

## Install

```sh
cargo install --path .
```

## Usage

```sh
anylist login
anylist sync
anylist untick <alias-or-item-id>
anylist logout
```

## zsh integration

Add this at the end of your `.zshrc`, after your plugins and key bindings:

```zsh
eval "$(anylist init zsh)"
```

`anylist init zsh` prints the widget code. `eval` runs that code in the current
shell so it can register the widget and key bindings. The widget uses your
current keymap and delegates to existing widget bindings outside AnyList
commands. `bindkey -s` string macros on those keys are unsupported.

Type `anylist untick ` followed by part of an item’s name, notes, alias, or ID.
Use Down and Up to preview a match. Esc restores the filter, and Backspace
restores it and deletes its last character. Enter runs the displayed command.
Tab completes subcommands and alias prefixes.
