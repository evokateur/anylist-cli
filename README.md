# AnyList CLI

A command-line client for [AnyList](https://www.anylist.com) shopping lists, built on [anylist_rs](https://github.com/phildenhoff/anylist_rs).

## Install

```sh
cargo install --path .
```

## Usage

```sh
anylist login    # prompt for email and password, then cache your lists
anylist sync     # fetch a fresh copy of your lists
anylist logout   # remove saved credentials and cached lists
```

`login` and `sync` print the account's default list, which is the list the CLI works on:

```console
$ anylist sync
Default list: Groceries
```

If you aren't logged in, or AnyList rejects the saved session, commands print `Not logged in. Run: anylist login` and exit with status 1.

## Local files

| File | Contents |
|---|---|
| `~/.local/share/anylist-cli/credentials.json` | Session tokens, mode `0600`. Your password is never saved. |
| `~/.cache/anylist-cli/lists.json` | Cached lists from the last `login` or `sync`. |

The `XDG_DATA_HOME` and `XDG_CACHE_HOME` environment variables override these locations. `logout` deletes both files and leaves `~/.config/anylist-cli/` untouched.

## Limitations

- Only one account is supported.
- Two commands that renew the session at the same moment can invalidate the saved refresh token. If that happens, run `anylist login` again.
