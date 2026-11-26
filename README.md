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

Add to `.zshrc`, after plugins and key bindings:

```zsh
eval "$(anylist init zsh)"
```

Tab completes subcommands and alias prefixes.

Text following `anylist untick ` filters candidates
- Selecting an item with down/up previews the command to untick it
- Esc restores the filter text
- Backspace restores the filter minus its last character
- Enter runs the displayed command


