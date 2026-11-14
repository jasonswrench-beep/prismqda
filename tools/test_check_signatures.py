# SPDX-License-Identifier: Apache-2.0
"""Tests for tools/check_signatures.py that ask Azure nothing and sign nothing."""
import contextlib
import io
import pathlib
import sys
import tempfile
import unittest
import zipfile
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import check_signatures as c  # noqa: E402

BASE = pathlib.Path(r"C:\unpacked")


def info(name, status="Valid", signer="Jason Wrench", timestamped=True):
    return {"path": str(BASE / name), "status": status, "signer": signer, "timestamped": timestamped}


class Judge(unittest.TestCase):
    def test_a_file_we_signed_and_timestamped_passes(self):
        problems, ours, makers = c.judge([info("prismqda-app.exe")], BASE, "Jason Wrench")
        self.assertEqual((problems, ours, makers), ([], 1, {}))

    def test_a_file_nobody_signed_is_a_problem_named_by_its_place(self):
        problems, ours, _ = c.judge([info("$PLUGINSDIR/System.dll", status="NotSigned", signer=None, timestamped=False)], BASE, "Jason Wrench")
        self.assertEqual(ours, 0)
        self.assertEqual(len(problems), 1)
        self.assertIn("$PLUGINSDIR/System.dll", problems[0])
        self.assertIn("not signed", problems[0])

    def test_a_signature_without_a_timestamp_is_a_problem(self):
        # The certificate lasts three days: without the timestamp the signature stops validating.
        problems, _, _ = c.judge([info("prismqda-app.exe", timestamped=False)], BASE, "Jason Wrench")
        self.assertEqual(len(problems), 1)
        self.assertIn("timestamp", problems[0])

    def test_a_file_someone_else_signed_validly_is_left_alone_and_counted(self):
        problems, ours, makers = c.judge([info("vendor.dll", signer="Microsoft Corporation")], BASE, "Jason Wrench")
        self.assertEqual((problems, ours, makers), ([], 0, {"Microsoft Corporation": 1}))

    def test_a_broken_signature_by_someone_else_is_still_a_problem(self):
        problems, _, _ = c.judge([info("vendor.dll", status="HashMismatch", signer="Microsoft Corporation")], BASE, "Jason Wrench")
        self.assertEqual(len(problems), 1)
        self.assertIn("HashMismatch", problems[0])


class Main(unittest.TestCase):
    def run_main(self, argv):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = c.main(argv)
        return code, out.getvalue()

    def test_nothing_to_check_prints_the_help_and_is_not_a_pass(self):
        code, text = self.run_main([])
        self.assertEqual(code, 2)
        self.assertIn("check_signatures.py", text)

    def test_a_file_that_is_not_there_is_named(self):
        code, text = self.run_main([r"C:\no\such\setup.exe"])
        self.assertEqual(code, 2)
        self.assertIn("There is no file at", text)

    def test_a_zip_whose_program_is_unsigned_fails(self):
        with tempfile.TemporaryDirectory() as d:
            archive = pathlib.Path(d) / "portable.zip"
            with zipfile.ZipFile(archive, "w") as z:
                z.writestr("PrismQDA/PrismQDA.exe", b"MZ not a real program")
                z.writestr("PrismQDA/LICENSE", "text")
            unsigned = lambda paths: [{"path": str(p), "status": "NotSigned", "signer": None, "timestamped": False} for p in paths]  # noqa: E731
            with mock.patch.object(c.sign_windows, "signatures", unsigned), mock.patch.dict("os.environ", {"OPENSTAGE_SIGN_PUBLISHER": "Jason Wrench"}):
                code, text = self.run_main(["--zip", str(archive)])
        self.assertEqual(code, 1)
        self.assertIn("PROBLEM", text)
        self.assertIn("PrismQDA.exe", text)

    def test_a_zip_with_no_program_at_all_fails(self):
        with tempfile.TemporaryDirectory() as d:
            archive = pathlib.Path(d) / "empty.zip"
            with zipfile.ZipFile(archive, "w") as z:
                z.writestr("PrismQDA/LICENSE", "text")
            with mock.patch.object(c.sign_windows, "signatures", lambda paths: []):
                code, text = self.run_main(["--zip", str(archive)])
        self.assertEqual(code, 1)
        self.assertIn("no program files", text)


if __name__ == "__main__":
    unittest.main()
