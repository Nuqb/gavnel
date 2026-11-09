"""Validate bundled locale files without third-party packages or network access."""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
BASES = {"en-GB": "en-US", "es-MX": "es-ES", "pt-PT": "pt-BR"}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"Duplicate translation key: {key}")
        result[key] = value
    return result


def main():
    catalogs = {
        path.stem: json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
        for path in (ROOT / "assets/locales").glob("*.json")
    }
    source = catalogs["en-US"]
    for locale, entries in catalogs.items():
        resolved = catalogs.get(BASES.get(locale), {}) | entries
        assert set(resolved) == set(source), f"{locale}: missing or unknown keys"
        for key, value in resolved.items():
            assert isinstance(value, str) and value.strip(), f"{locale}: empty {key}"
            assert "\ufffd" not in value, f"{locale}: invalid encoding in {key}"
            assert sorted(re.findall(r"\{[^{}]+\}", key)) == sorted(
                re.findall(r"\{[^{}]+\}", value)
            ), f"{locale}: placeholders changed in {key}"
    print(f"Validated {len(catalogs)} locales, {len(source)} messages each (including regional inheritance).")


if __name__ == "__main__":
    main()
