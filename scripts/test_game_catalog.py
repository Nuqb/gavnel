"""Regression checks for catalog import policy; no network required."""
import unittest
from update_game_catalog import catalog, safe_executable


def exe(name, **fields):
    return dict(name=name, os='win32', is_launcher=False, **fields)


def game(identifier, name, executables, **fields):
    return dict(id=identifier, name=name, executables=executables,
                themes=['Action'], third_party_skus=[{'distributor': 'steam', 'id': identifier}], **fields)


class CatalogTests(unittest.TestCase):
    def test_skips_runtimes_utilities_arguments_and_non_windows(self):
        for name in ['javaw.exe', 'game.exe', 'start.exe', 'client-win64-shipping.exe',
                     'uplayinstaller.exe', 'gameconfig.exe', 'updatemanager.exe',
                     'dotnetfx40_client_x86_x64.exe', '>special.exe']:
            self.assertFalse(safe_executable(exe(name)), name)
        self.assertFalse(safe_executable(exe('real.exe', arguments='required')))
        self.assertFalse(safe_executable(dict(name='real.exe', os='darwin', is_launcher=False)))
        self.assertFalse(safe_executable(dict(name='real.exe', os='win32', is_launcher=True)))
        self.assertTrue(safe_executable(exe('Game/bin/UniqueGame-Win64-Shipping.exe')))

    def test_reviewed_utilities_are_excluded_and_retirement_can_be_reproduced(self):
        records = [game('4', 'The Elder Scrolls III: Morrowind', [exe('morrowind.exe'), exe('mgexegui.exe')]), game('5', 'Fuse', [exe('fuse.exe')])]
        self.assertIn(('The Elder Scrolls III: Morrowind', 'morrowind.exe'), catalog(records))
        self.assertNotIn(('The Elder Scrolls III: Morrowind', 'mgexegui.exe'), catalog(records))
        self.assertNotIn(('Fuse', 'fuse.exe'), catalog(records))
        self.assertIn(('The Elder Scrolls III: Morrowind', 'mgexegui.exe'), catalog(records, apply_review=False))

    def test_conflicts_checked_against_excluded_records_and_aliases_deduplicated(self):
        entries = catalog([
            game('1', 'First', [exe('first/duplicate.exe'), exe('unique.exe'), exe('bin/UNIQUE.EXE')]),
            dict(id='2', name='Unclassified tool', executables=[exe('duplicate.exe')]),
            game('3', 'RPG Maker MV', [exe('rpgmv.exe')]),
        ])
        self.assertIn(('First', 'unique.exe'), entries)
        self.assertEqual(sum(e == 'unique.exe' for _, e in entries), 1)
        self.assertNotIn('duplicate.exe', [e for _, e in entries])
        self.assertNotIn('rpgmv.exe', [e for _, e in entries])


if __name__ == '__main__':
    unittest.main()
