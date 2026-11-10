import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("package_licenses", Path(__file__).with_name("package-licenses.py"))
package_licenses = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package_licenses)


class ReleaseLicenseTests(unittest.TestCase):
    def test_inventory_includes_transitive_runtime_and_build_crates_without_dev_only_crates(self):
        metadata = {
            "packages": [{"id": n, "name": n, "version": "1.0.0"} for n in ("app", "runtime", "build", "dev", "transitive")],
            "resolve": {"root": "app", "nodes": [
                {"id": "app", "deps": [{"pkg": n, "dep_kinds": [{"kind": k}]} for n, k in (("runtime", None), ("build", "build"), ("dev", "dev"))]},
                {"id": "runtime", "deps": [{"pkg": "transitive", "dep_kinds": [{"kind": None}]}]},
                {"id": "build", "deps": []}, {"id": "dev", "deps": []}, {"id": "transitive", "deps": []},
            ]},
        }
        selected = package_licenses.selected_packages(metadata)
        self.assertEqual({p["name"] for p in selected}, {"app", "runtime", "build", "transitive"})


if __name__ == "__main__":
    unittest.main()
