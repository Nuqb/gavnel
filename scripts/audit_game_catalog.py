"""Audit bundled game identities and aliases against Steam launch metadata.

Read-only audit. Responses are cached under work/game-audit for resumable runs.
Steam data is fetched through the public SteamCMD mirror, not from user accounts.
"""
from concurrent.futures import ThreadPoolExecutor, as_completed
from collections import Counter, defaultdict
import json
from pathlib import Path
import re
import time
import threading

from update_game_catalog import ROOT, REVIEW, basename, catalog

CACHE = ROOT / 'work/game-audit'
CACHE.mkdir(parents=True, exist_ok=True)
CONNECTIONS = threading.local()
STORE_RATE_LIMITED = threading.Event()


def fetch(appid):
    import requests  # Only online audits need this optional dependency.
    path = CACHE / f'{appid}.json'
    if path.exists():
        return json.loads(path.read_text(encoding='utf-8'))
    for attempt in range(3):
        try:
            if not hasattr(CONNECTIONS, 'session'):
                CONNECTIONS.session = requests.Session()
                CONNECTIONS.session.headers['User-Agent'] = 'GavnelCatalogAudit/1.0'
            response = CONNECTIONS.session.get(f'https://api.steamcmd.net/v1/info/{appid}', timeout=20)
            response.raise_for_status()
            info = response.json().get('data', {}).get(appid, {})
            result = dict(appid=appid, common=info.get('common', {}), launch=info.get('config', {}).get('launch', {}))
            path.write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
            time.sleep(0.1)
            return result
        except Exception as error:
            if attempt == 2:
                return dict(appid=appid, error=str(error))
            time.sleep(2 ** attempt)


def fetch_store(appid):
    import requests  # Offline classification tests do not fetch store data.
    path = CACHE / f'store-{appid}.json'
    if path.exists():
        return json.loads(path.read_text(encoding='utf-8'))
    if STORE_RATE_LIMITED.is_set():
        return dict(appid=appid, error='Store rate limit reached; retry a later audit run')
    if not hasattr(CONNECTIONS, 'session'):
        CONNECTIONS.session = requests.Session()
    try:
        response = CONNECTIONS.session.get('https://store.steampowered.com/api/appdetails',
            params=dict(appids=appid, filters='basic', l='english', cc='US'), timeout=20)
        if response.status_code == 429:
            STORE_RATE_LIMITED.set()
        response.raise_for_status()
        data = response.json().get(appid, {}).get('data', {})
        result = dict(appid=appid, name=data.get('name'), type=data.get('type'))
        path.write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
        time.sleep(0.5)
        return result
    except Exception as error:
        return dict(appid=appid, error=str(error))


def steam_ids(record):
    return {str(sku.get('id', '')).strip() for sku in record.get('third_party_skus', [])
            if sku.get('distributor') == 'steam' and str(sku.get('id', '')).strip().isdecimal()}


def windows_launches(info):
    launch = info.get('launch', {})
    values = launch.values() if isinstance(launch, dict) else launch if isinstance(launch, list) else []
    return [basename(dict(name=row.get('executable', ''))) for row in values
            if isinstance(row, dict) and 'windows' in row.get('config', {}).get('oslist', 'windows').split(',')]


def classify(entries, records, metadata):
    by_exe = defaultdict(list)
    for record in records:
        for executable in record.get('executables', []):
            if executable.get('os') == 'win32':
                by_exe[basename(executable)].append(record)
    results = []
    for name, executable in entries:
        sources = {record['id']: record for record in by_exe[executable.lower()]}.values()
        sources = [record for record in sources if record['name'] == name or len(by_exe[executable.lower()]) == 1]
        ids = sorted({appid for record in sources for appid in steam_ids(record)})
        ids = REVIEW.get('steam_app_ids', {}).get(name, ids)
        evidence = [metadata[appid] for appid in ids if appid in metadata]
        games = [row for row in evidence if row.get('common', {}).get('type', '').lower() in {'game', 'demo', 'mod', 'beta'}]
        exact = [row['appid'] for row in games if executable.lower() in windows_launches(row)]
        non_games = [row for row in evidence if row.get('common', {}).get('type', '').lower() in {'application', 'tool', 'video', 'music', 'advertising'}]
        if exact:
            status = 'game_and_launch_match'
        elif games:
            status = 'game_confirmed_alias_needs_review'
        elif non_games and len(non_games) == len(evidence):
            status = 'non_game_store_type'
        else:
            status = 'no_independent_store_confirmation'
        results.append(dict(name=name, executable=executable, status=status, steam_ids=ids,
                            exact_launch_ids=exact, store=[dict(id=row['appid'], name=row.get('common', {}).get('name'), type=row.get('common', {}).get('type'), launches=windows_launches(row)) for row in evidence]))
    return results


def main():
    records = json.loads((ROOT / 'work/detectable-games.json').read_text(encoding='utf-8-sig'))
    entries = catalog(records)
    selected = {exe.lower() for _, exe in entries}
    ids = sorted({appid for record in records if any(basename(exe) in selected for exe in record.get('executables', [])) for appid in steam_ids(record)})
    ids = sorted(set(ids) | {appid for values in REVIEW.get('steam_app_ids', {}).values() for appid in values})
    print(f'Auditing {len(entries)} aliases across {len(ids)} Steam app IDs', flush=True)
    metadata = {}
    started = time.monotonic()
    with ThreadPoolExecutor(max_workers=4) as pool:
        futures = {pool.submit(fetch, appid): appid for appid in ids}
        for future in as_completed(futures):
            metadata[futures[future]] = future.result()
            if len(metadata) % 100 == 0:
                print(f'{len(metadata)}/{len(ids)} checked in {time.monotonic()-started:.0f}s; {sum("error" in row for row in metadata.values())} fetch failures', flush=True)
    pending = [appid for appid, row in metadata.items()
               if not row.get('common', {}).get('type') or row.get('common', {}).get('type', '').lower() == 'config']
    print(f'Checking {len(pending)} missing classifications against Valve store metadata', flush=True)
    store_errors = []
    with ThreadPoolExecutor(max_workers=4) as pool:
        for store in pool.map(fetch_store, pending):
            if store.get('error'):
                store_errors.append(store)
            if store.get('type'):
                row = metadata[store['appid']]
                row['common'] = dict(row.get('common', {}), type=store['type'], name=store['name'])
                row['identity_source'] = 'Valve store appdetails'
    results = classify(entries, records, metadata)
    (CACHE / 'results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
    summary = dict(aliases=len(entries), titles=len({name for name, _ in entries}), statuses=dict(Counter(row['status'] for row in results)), fetch_errors=[row for row in metadata.values() if 'error' in row], store_errors=store_errors)
    (CACHE / 'summary.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
    print(json.dumps(summary), flush=True)


if __name__ == '__main__':
    main()
