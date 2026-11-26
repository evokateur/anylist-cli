# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "pyte>=0.8.2",
# ]
# ///
"""Exercise the installed zsh integration through a pseudo-terminal."""

import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import tempfile
import termios
import time
import unittest

import pyte

BEANS_A = "2b8f6e0c1a9d4c7eb53f8a2d6e1c0b97"
BEANS_B = "d07a5c3e9f2b4a18c6e0b7f1d3a9e542"
CREAM_CHEESE = "a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26"
BAGELS = "58d3b0e6f9a14c2d9e7b1a4c0f8e6d3b"
MILK = "4f1d2c9a8b7e4e3fa61c0d5b2e9f7a13"
UP, DOWN, ESC = "\x1b[A", "\x1b[B", "\x1b"


class TerminalCase(unittest.TestCase):
    shell_setup = "bindkey -e"

    def setUp(self):
        binary = Path(os.environ.get(
            "ANYLIST_TEST_BINARY",
            Path(__file__).resolve().parents[2] / "target/debug/anylist",
        )).resolve()
        self.assertTrue(binary.is_file() and binary.name == "anylist", f"Build anylist at {binary}")
        fixture = tempfile.TemporaryDirectory()
        self.addCleanup(fixture.cleanup)
        root = Path(fixture.name)
        self.credentials = root / "data/anylist-cli/credentials.json"
        self.config = root / "config/anylist-cli/config.toml"
        self.cache = root / "cache/anylist-cli/lists.json"
        for directory in ("data", "cache", "config"):
            (root / directory / "anylist-cli").mkdir(parents=True)
        self.credentials.write_text(json.dumps({
            "access_token": "fixture", "refresh_token": "fixture",
            "user_id": "fixture", "is_premium_user": False,
        }))
        items = [
            (MILK, "Milk", "Organic, whole"),
            (BEANS_A, "Coffee beans", "Whole bean"),
            (BEANS_B, "Coffee beans", "Whole bean"),
            (CREAM_CHEESE, "Cream cheese", "Philadelphia"),
            (BAGELS, "Bagels", "For Phil's brunch"),
            ("a", "Apple", ""),
            ("b", "Pear", ""),
        ]
        self.cache.write_text(json.dumps({
            "current_list_id": "market",
            "lists": [{
                "id": "market", "name": "queste sono le cose",
                "items": [
                    {"id": key, "name": name, "notes": notes, "ticked": True}
                    for key, name, notes in items
                ],
            }],
        }))
        self.config.write_text(
            f'[aliases]\nphilly = "{CREAM_CHEESE}"\n'
            'mixed-apple = "a"\nmixed-pear = "b"\n'
        )
        self.screen = pyte.Screen(80, 24)
        self.stream = pyte.Stream(self.screen)
        self.pid, self.terminal = pty.fork()
        if self.pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
            environment = {
                **os.environ, "TERM": "xterm-256color",
                "PATH": f"{binary.parent}{os.pathsep}{os.environ['PATH']}",
                "XDG_DATA_HOME": str(root / "data"),
                "XDG_CACHE_HOME": str(root / "cache"),
                "XDG_CONFIG_HOME": str(root / "config"),
                "ZDOTDIR": str(root),
            }
            os.execvpe("zsh", ["zsh", "-f"], environment)
        self.addCleanup(self.close_terminal)
        self.send(f"PROMPT='TEST> '; KEYTIMEOUT=10; {self.shell_setup}; "
                  """setup=$(anylist init zsh) && eval "$setup" && print -r -- WIDGET-'READY'\n""")
        self.expect("WIDGET-READY\r\n")
        self.drain()

    def close_terminal(self):
        os.kill(self.pid, signal.SIGKILL)
        os.waitpid(self.pid, 0)
        os.close(self.terminal)

    def send(self, text):
        os.write(self.terminal, text.encode())

    def read_output(self):
        output = os.read(self.terminal, 65536).decode(errors="replace")
        self.stream.feed(output)
        return output

    def expect(self, text):
        output = ""
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if select.select([self.terminal], [], [], 0.1)[0]:
                output += self.read_output()
                if text in output:
                    return output
        self.fail(f"Missing {text!r} in terminal output: {output!r}")

    def drain(self):
        output = ""
        while select.select([self.terminal], [], [], 0.1)[0]:
            output += self.read_output()
        return output

    def expect_display(self, text):
        deadline = time.monotonic() + 5
        self.drain()
        while time.monotonic() < deadline:
            display = "\n".join(self.screen.display)
            if text in display:
                return display
            if select.select([self.terminal], [], [], 0.1)[0]:
                self.read_output()
        self.fail(f"Missing {text!r} on screen:\n{display}")

    def assert_prompt(self, command, rows=()):
        prompt = f"TEST> {command}"
        expected = [prompt.rstrip(), *rows]
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            self.drain()
            lines = [line.rstrip() for line in self.screen.display]
            while lines and not lines[-1]:
                lines.pop()
            prompt_row = len(lines) - len(expected)
            cursor = (self.screen.cursor.y, self.screen.cursor.x)
            if lines[prompt_row:] == expected and cursor == (prompt_row, len(prompt)):
                return
        self.fail(f"Expected {expected!r} with cursor at end of prompt. "
                  f"Screen at timeout with cursor at {cursor}:\n" + "\n".join(lines))


class InteractionTest(TerminalCase):
    def test_alias_shown_and_filtered_without_ids(self):
        self.send("anylist untick phil")
        self.assert_prompt("anylist untick phil", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send("\x15anylist untick lly")
        self.assert_prompt("anylist untick lly", [
            "  Cream cheese  |  Philadelphia  [philly]",
        ])


    def test_down_previews_alias_then_id_and_escape_restores(self):
        self.send("anylist untick phil")
        self.expect_display("[philly]")
        self.send(DOWN)
        self.assert_prompt("anylist untick philly", [
            "> Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send(DOWN)
        self.assert_prompt(f"anylist untick {BAGELS}", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "> Bagels  |  For Phil's brunch",
        ])
        self.send(ESC)
        self.assert_prompt("anylist untick phil", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])

    def test_up_out_of_list_restores_typed_text(self):
        self.send("anylist untick phil")
        self.expect_display("[philly]")
        self.send(DOWN + DOWN + UP)
        self.assert_prompt("anylist untick philly", [
            "> Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send(UP)
        self.assert_prompt("anylist untick phil", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])

    def test_enter_runs_previewed_command_and_history_keeps_it(self):
        self.send("anylist untick bean")
        self.expect_display("Coffee beans")
        self.send(DOWN + DOWN)
        self.expect_display(f"anylist untick {BEANS_B}")
        self.credentials.unlink()
        self.send("\r")
        self.expect("Not logged in. Run: anylist login")
        self.drain()
        self.send(UP)
        self.assert_prompt(f"anylist untick {BEANS_B}")

    def test_preview_is_read_only_until_leaving_list(self):
        self.send("anylist untick phil")
        self.expect_display("[philly]")
        self.send(DOWN + "\x7f" + "x" + "\x15" + "\x17")
        self.assert_prompt("anylist untick philly", [
            "> Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send(ESC)
        self.assert_prompt("anylist untick phil", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send("\x7f")
        self.assert_prompt("anylist untick phi", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])

    def test_enter_runs_typed_text_verbatim(self):
        self.send("anylist untick phil")
        self.expect_display("[philly]")
        self.send("\n")
        self.expect("No item with alias or ID 'phil' in 'queste sono le cose'")

    def test_tab_completes_public_subcommands_and_aliases(self):
        for prefix, command in [("lo", "log"), ("logi", "login "), ("logo", "logout "),
                                ("sy", "sync "), ("un", "untick ")]:
            self.send(f"\x15anylist {prefix}\t")
            self.assert_prompt(f"anylist {command}")
        self.send("ph\t")
        self.assert_prompt("anylist untick philly", [
            "  Cream cheese  |  Philadelphia  [philly]",
        ])
        self.send("\x15anylist untick mix\t")
        self.assert_prompt("anylist untick mixed-", [
            "  Apple  |    [mixed-apple]",
            "  Pear  |    [mixed-pear]",
        ])

    def test_tab_does_not_complete_ids_or_internal_commands(self):
        self.send("anylist untick bag\t")
        self.assert_prompt("anylist untick bag", ["  Bagels  |  For Phil's brunch"])
        for prefix in ["in", "cand"]:
            self.send(f"\x15anylist {prefix}\t")
            self.assert_prompt(f"anylist {prefix}")

    def test_tab_at_an_empty_argument_completes_a_unique_alias(self):
        self.config.write_text(f'[aliases]\nphilly = "{CREAM_CHEESE}"\n')
        self.send("anylist untick ")
        self.assert_prompt("anylist untick ")
        self.send("\t")
        self.assert_prompt("anylist untick philly", [
            "  Cream cheese  |  Philadelphia  [philly]",
        ])

    def test_down_cannot_select_an_item_when_tab_leaves_the_menu_closed(self):
        self.send("anylist untick \t" + DOWN)
        self.assert_prompt("anylist untick ")

    def test_hyphen_alias_completion_and_preview_produce_a_valid_argument(self):
        self.config.write_text(f'[aliases]\n-philly = "{CREAM_CHEESE}"\n')
        self.send("anylist untick -ph\t")
        self.assert_prompt("anylist untick -- -philly", [
            "  Cream cheese  |  Philadelphia  [-philly]",
        ])
        self.send("\x15anylist untick phil" + DOWN)
        self.assert_prompt("anylist untick -- -philly", [
            "> Cream cheese  |  Philadelphia  [-philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.credentials.unlink()
        self.send("\r")
        self.expect("Not logged in. Run: anylist login")

    def test_bare_id_in_notes_cannot_become_a_selectable_row(self):
        snapshot = json.loads(self.cache.read_text())
        items = snapshot["lists"][0]["items"]
        next(item for item in items if item["id"] == CREAM_CHEESE)["notes"] += f"\n{MILK}"
        self.cache.write_text(json.dumps(snapshot))
        self.send("anylist untick cream" + DOWN + DOWN)
        self.assert_prompt("anylist untick philly", [
            "> Cream cheese  |  Philadelphia  [philly]",
        ])

    def test_navigation_keeps_the_last_results_until_the_query_changes(self):
        self.send("anylist untick phil")
        self.assert_prompt("anylist untick phil", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.credentials.unlink()
        self.send(" " + DOWN)
        self.assert_prompt("anylist untick philly", [
            "> Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send(ESC)
        self.assert_prompt("anylist untick phil ", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send("x")
        self.assert_prompt("anylist untick phil x")

    def test_regular_shell_tab_completion_still_works(self):
        self.send("prin\t")
        self.assert_prompt("print")

    def test_no_match_names_the_list(self):
        self.send("anylist untick absent")
        self.assert_prompt("anylist untick absent", [
            "No matching item in 'queste sono le cose'",
        ])

    def test_trailing_shell_code_runs_as_written(self):
        self.send('anylist untick phil; print -r -- "AFTER-COMMAND"')
        self.expect_display("No matching item")
        self.send("\r")
        output = self.expect("AFTER-COMMAND\r\n")
        self.assertIn("No item with alias or ID 'phil' in 'queste sono le cose'", output)


class DelegationTest(TerminalCase):
    shell_setup = (
        "bindkey -e; "
        "append_key() { BUFFER+=${KEYS[-1]}; CURSOR=$#BUFFER; }; "
        "zle -N append_key; "
        "for key in '^I' '^M' '^J' '^[[A' '^[[B' '^[OA' '^[OB' '^['; "
        'do bindkey "$key" append_key; done; '
        "report_key() { BUFFER+=x; CURSOR=$#BUFFER; }; zle -N report_key; "
        "for key in '^I' '^M' '^J' '^['; do bindkey \"$key\" report_key; done"
    )

    def test_keys_call_the_widgets_bound_before_installation(self):
        self.send("other\t\r\n" + UP + DOWN + "\x1bOA\x1bOB" + ESC)
        self.assert_prompt("otherxxxABABx")


class ViModeTest(TerminalCase):
    shell_setup = "bindkey -v"

    def test_escape_restores_preview_then_enters_command_mode(self):
        self.send("anylist untick phil" + DOWN)
        self.expect_display("> Cream cheese")
        self.send(ESC)
        self.assert_prompt("anylist untick phil", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])
        self.send(ESC)
        time.sleep(0.15)
        self.send("0")
        self.drain()
        self.assertEqual(self.screen.cursor.x, len("TEST> "))
        self.assertNotIn("[philly]", "\n".join(self.screen.display))


class KshArraysTest(TerminalCase):
    shell_setup = "bindkey -e; setopt ksharrays"

    def test_preview_targets_the_highlighted_row_with_ksh_arrays_enabled(self):
        self.send("anylist untick phil" + DOWN)
        self.assert_prompt("anylist untick philly", [
            "> Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])


if __name__ == "__main__":
    unittest.main()
