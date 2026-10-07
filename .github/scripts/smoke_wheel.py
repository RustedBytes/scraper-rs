"""Check and install exactly one wheel, then import it outside the source tree."""

from pathlib import Path
import subprocess
import sys
import sysconfig
import tempfile
import zipfile

wheel_dir, expected_python, expected_abi = sys.argv[1:]
actual_python = f"{sys.version_info.major}.{sys.version_info.minor}"
if sysconfig.get_config_var("Py_GIL_DISABLED"):
    actual_python += "t"
assert actual_python == expected_python, (actual_python, expected_python)
wheels = list(Path(wheel_dir).resolve().glob("*.whl"))
assert len(wheels) == 1, wheels
wheel = wheels[0]
python_tag, abi_tag, platform_tag = wheel.stem.rsplit("-", 3)[1:]
expected_abis = {"abi3", "abi3t"} if expected_abi == "abi3t" else {expected_abi}
assert set(abi_tag.split(".")) == expected_abis, wheel.name
expected_tag = "cp315" if expected_abi == "abi3t" else "cp314"
assert python_tag == expected_tag, wheel.name
with zipfile.ZipFile(wheel) as archive:
    metadata = [
        name for name in archive.namelist() if name.endswith(".dist-info/WHEEL")
    ]
    assert len(metadata) == 1, metadata
    tags = {
        line.removeprefix("Tag: ")
        for line in archive.read(metadata[0]).decode().splitlines()
        if line.startswith("Tag: ")
    }
    expected_tags = {
        f"{python_tag}-{abi}-{platform}"
        for abi in expected_abis
        for platform in platform_tag.split(".")
    }
    assert tags == expected_tags, (tags, expected_tags)
print(wheel.name, flush=True)
subprocess.run(
    [
        sys.executable,
        "-m",
        "pip",
        "install",
        "--disable-pip-version-check",
        "--no-index",
        "--no-deps",
        "--force-reinstall",
        str(wheel),
    ],
    check=True,
)
# -I prevents the checkout's scraper_rs package from masking the installed wheel.
subprocess.run(
    [
        sys.executable,
        "-I",
        "-c",
        """
import sys
import scraper_rs
assert scraper_rs.__version__
with scraper_rs.Document('<p id="smoke">wheel works</p>') as doc:
    assert doc.select_first('#smoke').text == 'wheel works'
    assert doc.xpath_first('//p').text == 'wheel works'
if hasattr(sys, '_is_gil_enabled'):
    import sysconfig
    if sysconfig.get_config_var('Py_GIL_DISABLED'):
        assert not sys._is_gil_enabled(), 'Extension re-enabled the GIL'
print(sys.version, scraper_rs.__version__, scraper_rs.__file__)
""",
    ],
    cwd=tempfile.gettempdir(),
    check=True,
)
