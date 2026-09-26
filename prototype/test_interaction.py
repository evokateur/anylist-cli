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


class InteractionTest(unittest.TestCase):
    """Verify selection and execution stay separate and preserve item identity."""

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

    def test_tab_clears_menu_without_corrupting_prompt(self):
        self.send("anylist untick beans")
        self.expect_display("[beans-b]")
        self.assert_prompt("anylist untick beans", [
            "> Coffee beans  |  Whole bean  [beans-a]",
            "  Coffee beans  |  Whole bean  [beans-b]",
        ])
        self.send("\t")
        self.assert_prompt("anylist untick beans-a")

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
        self.send("\x15anylist untick beans")
        self.expect_display("[beans-b]")
        self.send("\t")
        self.assert_prompt("anylist untick beans-a")
        row = self.screen.cursor.y
        self.assertFalse(any(cell.underscore for cell in self.screen.buffer[row].values()))

    def assert_prompt(self, command, rows=()):
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

    def test_editing_completed_command_reopens_menu(self):
        self.send("anylist untick beans")
        self.expect_display("[beans-b]")
        self.send("\t")
        self.assert_prompt("anylist untick beans-a")
        self.send("\x7f")
        self.expect_display("[beans-b]")
        self.send("\x1b[B")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-b]")
        self.send("\t")
        self.assert_prompt("anylist untick beans-b")
        self.send("\r")
        self.expect("Resolved command: anylist untick beans-b")
        self.assert_prompt("")

    def test_tab_clears_wrapped_rows_in_narrow_terminal(self):
        self.screen.resize(lines=24, columns=32)
        fcntl.ioctl(self.terminal, termios.TIOCSWINSZ,
                    struct.pack("HHHH", 24, 32, 0, 0))
        self.drain()
        self.send("anylist untick beans")
        self.expect_display("Whole bean")
        self.send("\t")
        self.assert_prompt("anylist untick beans-a")

    def test_tab_accepts_first_without_executing(self):
        self.send("anylist untick MIL ")
        self.expect_display("> Milk  |  Organic, whole  [milk]")
        self.drain()
        self.send("\t")
        output = self.expect_display("TRACER> anylist untick milk")
        self.assertNotIn("DEMO: Unticked:", output)
        self.send("\n")
        output = self.expect("Resolved command: anylist untick milk")
        self.assertIn("DEMO: Unticked: Milk [milk]", output)
        self.send("anylist untick milk\n")
        self.expect("DEMO: Unticked: Milk [milk]")

    def test_enter_substitutes_first_match_and_executes(self):
        self.send("anylist untick MIL ")
        self.expect_display("> Milk  |  Organic, whole  [milk]")
        self.drain()
        self.send("\r")
        output = self.expect("Resolved command: anylist untick milk")
        self.assertIn("DEMO: Unticked: Milk [milk]", output)
        self.assertIn("anylist untick milk", output)

    def test_unticked_items_remain_searchable_and_untick_is_idempotent(self):
        self.send("anylist untick mil")
        self.expect_display("> Milk  |  Organic, whole  [milk]")
        self.send("\r")
        self.expect("Resolved command: anylist untick milk")
        self.drain()
        self.send("anylist untick mil")
        self.expect_display("> Milk  |  Organic, whole  [milk]")
        self.send("\r")
        output = self.expect("Resolved command: anylist untick milk")
        self.assertIn("DEMO: Unticked: Milk [milk]", output)
        self.assertNotIn("Already unticked", output)

    def test_arrows_select_second_duplicate_by_id(self):
        self.send("anylist untick bean")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-a]")
        self.drain()
        self.send("\x1b[B")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-b]")
        self.send("\x1b[A")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-a]")
        self.send("\x1b[B")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-b]")
        self.send("\n")
        output = self.expect("Resolved command: anylist untick beans-b")
        self.assertIn("DEMO: Unticked: Coffee beans [beans-b]", output)
        self.send("anylist untick bean")
        output = self.expect_display("  Coffee beans  |  Whole bean  [beans-b]")
        self.assertIn("> Coffee beans  |  Whole bean  [beans-a]", output)

    def test_notes_filter_and_backspace(self):
        self.send("anylist untick bergamotx")
        self.expect_display("No matching item in 'queste sono le cose'")
        self.send("\x7f")
        self.expect_display("> Earl Grey tea  |  Sam's favourite; bergamot  [tea]")
        self.send("\n")
        output = self.expect("Resolved command: anylist untick tea")
        self.assertIn("DEMO: Unticked: Earl Grey tea [tea]", output)

    def test_unselected_input_and_cancel_do_not_untick(self):
        self.send("anylist untick no-such-item")
        self.expect_display("No matching item in 'queste sono le cose'")
        self.drain()
        self.send("\t\n")
        output = self.expect("No item with ID 'no-such-item' in queste sono le cose")
        self.assertNotIn("DEMO: Unticked:", output)
        self.send('print -r -- "EXIT:$?"\n')
        self.expect("EXIT:1\r\n")
        self.send("anylist untick mil")
        self.expect_display("> Milk  |  Organic, whole  [milk]")
        self.send("\x03")
        self.drain()
        self.send("anylist untick mil")
        self.expect_display("> Milk  |  Organic, whole  [milk]")

    def test_filter_change_resets_focus_and_history_keeps_identity(self):
        self.send("anylist untick bean")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-a]")
        self.drain()
        self.send("\x1b[B")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-b]")
        self.send("s")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-a]")
        self.send("\r")
        self.expect("Resolved command: anylist untick beans-a")
        self.drain()
        self.send("\x1b[A")
        self.expect_display("TRACER> anylist untick beans-a")
        self.send("\r")
        self.expect("DEMO: Unticked: Coffee beans [beans-a]")

    def test_empty_filter_and_regular_shell_commands(self):
        self.send("anylist untick   ")
        output = self.expect("anylist untick") + self.drain()
        self.assertNotIn("Groceries (sample data)", output)
        self.assertNotIn("[milk]", output)
        self.assertNotIn("No matching item", output)
        self.send("\t\r")
        output = self.drain()
        self.assertNotIn("DEMO:", output)
        self.assertNotIn("Groceries (sample data)", output)
        self.assertIn("Usage: anylist untick <item_id>", output)
        self.send("anylist untick m")
        self.expect_display("> Milk  |  Organic, whole  [milk]")
        self.drain()
        self.send("\x7f")
        output = self.drain()
        self.assertNotIn("[milk]", output)
        self.assertNotIn("Groceries (sample data)", output)
        self.assertNotIn("No matching item", output)
        self.send("\r")
        self.expect("Usage: anylist untick <item_id>")
        self.drain()
        self.send("print -r -- NORMAL-SHELL\r")
        self.expect("NORMAL-SHELL\r\n")

    def test_typed_id_filters_candidates(self):
        self.send("anylist untick beans-b")
        self.expect_display("> Coffee beans  |  Whole bean  [beans-b]")
        self.drain()
        self.send("\r")
        output = self.expect("Resolved command: anylist untick beans-b")
        self.assertIn("DEMO: Unticked: Coffee beans [beans-b]", output)
        self.assertNotIn("DEMO: Unticked: Coffee beans [beans-a]", output)

    def test_command_without_selection_runs_as_written(self):
        self.send('anylist untick beans-b; print -r -- "AFTER-COMMAND"')
        self.expect_display("No matching item in 'queste sono le cose'")
        self.drain()
        self.send("\r")
        output = self.expect("AFTER-COMMAND\r\n")
        self.assertIn("Resolved command: anylist untick beans-b", output)


if __name__ == "__main__":
    unittest.main()
