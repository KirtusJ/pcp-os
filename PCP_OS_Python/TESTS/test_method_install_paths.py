"""Prevent sealed ACR method installs from following pre-existing destination links."""
import os
from pathlib import Path
import tempfile
import unittest
from pcpos.base import PcposError, INTEGRITY
from pcpos.method import install_from_zip

PACKET = Path(os.environ.get('PCP_TEST_METHOD_ZIP', Path(__file__).resolve().parents[2] / 'ACR_1.0_Implementation.zip')).resolve()

class MethodInstallSafety(unittest.TestCase):
    def setUp(self):
        if not PACKET.is_file():
            self.skipTest('set PCP_TEST_METHOD_ZIP to the pinned sealed ACR packet')
        tmp=tempfile.TemporaryDirectory(prefix='pcp-method-preflight-')
        self.addCleanup(tmp.cleanup)
        self.root=Path(tmp.name)

    def link(self,path,target):
        try:
            path.symlink_to(target,target_is_directory=True)
        except (OSError,NotImplementedError) as e:
            self.skipTest(f'cannot create a symlink: {e}')

    def test_normal_install_has_verified_identity(self):
        parent=self.root/'normal'; parent.mkdir()
        dest, ident=install_from_zip(PACKET,parent)
        self.assertTrue(ident['matches_required'])
        self.assertTrue((dest/'MODULES/01_Core/SOURCE/Specification.md').is_file())

    def test_preexisting_modules_directory_link_is_refused_without_exterior_write(self):
        parent=self.root/'destination'; parent.mkdir()
        dest=parent/'ACR_1.0_Implementation'; dest.mkdir()
        outside=self.root/'exterior'; outside.mkdir()
        self.link(dest/'MODULES',outside)
        with self.assertRaises(PcposError) as caught:
            install_from_zip(PACKET,parent)
        self.assertEqual(caught.exception.code,INTEGRITY)
        self.assertEqual(list(outside.rglob('*')), [])
        self.assertFalse((dest/'MANIFEST.json').exists())

    def test_destination_root_link_is_refused_without_exterior_write(self):
        parent=self.root/'destination'; parent.mkdir()
        outside=self.root/'exterior'; outside.mkdir()
        self.link(parent/'ACR_1.0_Implementation',outside)
        with self.assertRaises(PcposError): install_from_zip(PACKET,parent)
        self.assertEqual(list(outside.rglob('*')), [])

    def test_prelinked_leaf_path_is_refused_without_exterior_write(self):
        parent=self.root/'destination'; parent.mkdir()
        dest=parent/'ACR_1.0_Implementation'; (dest/'GUIDES').mkdir(parents=True)
        outside=self.root/'exterior'; outside.write_bytes(b'SENTINEL')
        self.link(dest/'GUIDES'/'Entry_Card.md',outside)
        with self.assertRaises(PcposError): install_from_zip(PACKET,parent)
        self.assertEqual(outside.read_bytes(),b'SENTINEL')
        self.assertFalse((dest/'MANIFEST.json').exists())

if __name__ == '__main__':unittest.main(verbosity=2)
