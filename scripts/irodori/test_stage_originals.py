import copy
from pathlib import Path
import sys
import tempfile
import unittest
from stage_originals import stage


@unittest.skipUnless(sys.platform == 'darwin', 'clonefile requires macOS')
class StagingTest(unittest.TestCase):
    def test_clone_is_independent_and_existing_output_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / 'original'
            source.write_bytes(b'original bytes')
            roles = {role: dict(revision='a' * 40, files=[dict(logical_name='file',
                     original_path=str(source), bytes=source.stat().st_size)])
                     for role in ('model', 'codec', 'tokenizer', 'watermark')}
            inventory = dict(format_version=1, roles=roles)
            output = root / 'next'
            report = stage(inventory, output)
            target = Path(report['files'][0]['path'])
            self.assertNotEqual(source.stat().st_ino, target.stat().st_ino)
            target.write_bytes(b'independent edit')
            self.assertEqual(source.read_bytes(), b'original bytes')
            with self.assertRaises(FileExistsError):
                stage(inventory, output)
            self.assertEqual(target.read_bytes(), b'independent edit')
            invalid = copy.deepcopy(inventory)
            invalid['roles']['model']['files'][0]['logical_name'] = '../escape'
            with self.assertRaises(ValueError):
                stage(invalid, root / 'invalid')
            self.assertFalse((root / 'invalid').exists())


if __name__ == '__main__':
    unittest.main()
