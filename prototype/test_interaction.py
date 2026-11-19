# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "pyte>=0.8.2",
# ]
# ///
"""Exercise the tracer's real zsh key bindings through a pseudo-terminal."""

import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
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


class InteractionTest(unittest.TestCase):
    """Verify Enter runs the line as shown and navigation previews aliases or IDs."""

    def setUp(self):
        self.screen = pyte.Screen(80, 24)
        self.stream = pyte.Stream(self.screen)
        self.pid, self.terminal = pty.fork()
        if self.pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
            os.chdir(Path(__file__).parent)
            os.environ["TERM"] = "xterm-256color"
            os.execvp("zsh", ["zsh", "-f"])
        self.send("PROMPT='TRACER> '; source ./anylist.zsh\n")
        self.expect("Exit leaves the demo shell.")
        self.drain()

    def tearDown(self):
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
        # ESC waits for KEYTIMEOUT before zle treats it as a lone key.
        time.sleep(0.5)
        self.drain()
        lines = [line.rstrip() for line in self.screen.display]
        prompt = f"TRACER> {command}"
        self.assertIn(prompt.rstrip(), lines, "\n".join(lines))
        prompt_row = max(index for index, line in enumerate(lines) if line == prompt.rstrip())
        visible = lines[prompt_row:]
        while visible and not visible[-1]:
            visible.pop()
        self.assertEqual(visible, [prompt.rstrip(), *rows])
        self.assertEqual((self.screen.cursor.y, self.screen.cursor.x),
                         (prompt_row, len(prompt)))

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

    def test_enter_runs_typed_text_verbatim(self):
        self.send("anylist untick phil")
        self.expect_display("[philly]")
        self.send("\r")
        output = self.expect("No item with alias or ID 'phil' in queste sono le cose")
        self.assertNotIn("DEMO: Unticked:", output)

    def test_tab_completes_subcommand_and_alias(self):
        self.send("anylist un\t")
        self.assert_prompt("anylist untick ")
        self.send("ph\t")
        self.assert_prompt("anylist untick philly", [
            "  Cream cheese  |  Philadelphia  [philly]",
        ])
        self.send("\r")
        self.expect(f"DEMO: Unticked: Cream cheese ({CREAM_CHEESE}) on queste sono le cose")

    def test_tab_without_alias_match_does_nothing(self):
        self.send("anylist untick bag\t")
        self.assert_prompt("anylist untick bag", ["  Bagels  |  For Phil's brunch"])

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
        self.send("\r")
        output = self.expect(f"DEMO: Unticked: Coffee beans ({BEANS_B})")
        self.assertNotIn(BEANS_A, output)
        self.drain()
        self.send(UP)
        self.assert_prompt(f"anylist untick {BEANS_B}", ["  Coffee beans  |  Whole bean"])
        self.send("\r")
        self.expect(f"DEMO: Unticked: Coffee beans ({BEANS_B})")

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

    def test_typed_id_filters_and_runs(self):
        self.send(f"anylist untick {MILK}")
        self.expect_display("  Milk  |  Organic, whole")
        self.send("\r")
        self.expect(f"DEMO: Unticked: Milk ({MILK})")

    def test_untick_is_idempotent(self):
        for _ in range(2):
            self.send("anylist untick philly\r")
            output = self.expect(f"DEMO: Unticked: Cream cheese ({CREAM_CHEESE})")
            self.assertNotIn("Already unticked", output)

    def test_no_match_title_is_single_quoted(self):
        self.send("anylist untick absent")
        message = "No matching item in 'queste sono le cose'"
        self.expect_display(message)
        self.assert_prompt("anylist untick absent", [message])
        row = self.screen.cursor.y + 1
        for column in range(len(message)):
            cell = self.screen.buffer[row][column]
            self.assertFalse(cell.underscore)
            self.assertFalse(cell.italics)

    def test_notes_filter_and_backspace(self):
        self.send("anylist untick bergamotx")
        self.expect_display("No matching item in 'queste sono le cose'")
        self.send("\x7f")
        self.expect_display("  Earl Grey tea  |  Sam's favourite; bergamot  [earl]")

    def test_menu_rows_erase_in_narrow_terminal(self):
        self.screen.resize(lines=24, columns=32)
        fcntl.ioctl(self.terminal, termios.TIOCSWINSZ,
                    struct.pack("HHHH", 24, 32, 0, 0))
        self.drain()
        self.send("anylist untick bean")
        self.expect_display("Whole bean")
        self.send("\x15")
        self.assert_prompt("")

    def test_cancel_does_not_untick(self):
        self.send("anylist untick phil")
        self.expect_display("[philly]")
        self.send(DOWN + "\x03")
        output = self.drain()
        self.assertNotIn("DEMO: Unticked:", output)
        self.send("anylist untick phil")
        self.assert_prompt("anylist untick phil", [
            "  Cream cheese  |  Philadelphia  [philly]",
            "  Bagels  |  For Phil's brunch",
        ])

    def test_empty_filter_and_regular_shell_commands(self):
        self.send("anylist untick   ")
        self.assert_prompt("anylist untick   ")
        self.send("\t\r")
        self.expect("Usage: anylist untick <alias|item_id>")
        self.send("anylist untick m")
        self.expect_display("  Milk  |  Organic, whole")
        self.send("\x7f")
        self.assert_prompt("anylist untick ")
        self.send("\x15print -r -- NORMAL-SHELL\r")
        self.expect("NORMAL-SHELL\r\n")

    def test_command_with_trailing_shell_code_runs_as_written(self):
        self.send('anylist untick philly; print -r -- "AFTER-COMMAND"')
        self.expect_display("No matching item in 'queste sono le cose'")
        self.send("\r")
        output = self.expect("AFTER-COMMAND\r\n")
        self.assertIn("DEMO: Unticked: Cream cheese", output)


if __name__ == "__main__":
    unittest.main()
