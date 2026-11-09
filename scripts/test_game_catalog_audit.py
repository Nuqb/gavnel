"""Offline checks for evidence classification; does not query public services."""
import unittest
from audit_game_catalog import classify, windows_launches, steam_ids


class AuditTests(unittest.TestCase):
    def test_source_ids_are_trimmed_and_invalid_ids_skipped(self):
        record = dict(third_party_skus=[dict(distributor='steam',id='1125510\t'),dict(distributor='steam',id=None)])
        self.assertEqual(steam_ids(record), {'1125510'})

    def test_windows_launches_ignores_linux_and_mac_entries(self):
        info = dict(launch={'0':dict(executable='bin/Game.exe',config=dict(oslist='windows')),
                            '1':dict(executable='mac/Game.app',config=dict(oslist='macos')),
                            '2':dict(executable='linux/Game',config=dict(oslist='linux'))})
        self.assertEqual(windows_launches(info), ['game.exe'])

    def test_store_type_and_executable_evidence_are_distinct(self):
        records=[dict(id='1',name='Game',executables=[dict(os='win32',name='game1.exe')],
                      third_party_skus=[dict(distributor='steam',id='10')])]
        metadata={'10':dict(appid='10',common=dict(name='Game',type='Game'),launch={'0':dict(executable='game1.exe')})}
        row=classify([('Game','game1.exe')],records,metadata)[0]
        self.assertEqual(row['status'],'game_and_launch_match')
        metadata['10']['launch']={'0':dict(executable='other.exe')}
        self.assertEqual(classify([('Game','game1.exe')],records,metadata)[0]['status'],'game_confirmed_alias_needs_review')
        metadata['10']['common']['type']='Application'
        self.assertEqual(classify([('Game','game1.exe')],records,metadata)[0]['status'],'non_game_store_type')
        metadata['10']['common']={}
        self.assertEqual(classify([('Game','game1.exe')],records,metadata)[0]['status'],'no_independent_store_confirmation')


if __name__ == '__main__':
    unittest.main()
