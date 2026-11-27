"""Verify a silent NSIS install and the Stockfish sidecar on a Windows CI runner."""

import argparse
from pathlib import Path
import subprocess
import tempfile


def verify(installer):
    with tempfile.TemporaryDirectory(prefix="engineroom-nsis-") as tmp:
        destination = Path(tmp) / "install"
        # NSIS requires /D to be last and unquoted, even with spaces in the path.
        subprocess.run(
            f'"{installer.resolve(strict=True)}" /S /D={destination}',
            check=True,
            timeout=120,
        )
        app = destination / "engineroom-rune.exe"
        sidecar = destination / "stockfish.exe"
        try:
            if not app.is_file() or not sidecar.is_file():
                raise RuntimeError(f"Incomplete NSIS install in {destination}")
            stockfish = subprocess.run(
                [sidecar], input="uci\nquit\n", capture_output=True,
                text=True, check=True, timeout=30,
            )
            if "id name Stockfish 19" not in stockfish.stdout or "uciok" not in stockfish.stdout:
                raise RuntimeError(f"Bundled Stockfish failed its UCI handshake:\n{stockfish.stdout}")
            tree = Path(tmp) / "home.json"
            subprocess.run(
                [app, "--tree", tree], cwd=destination, check=True, timeout=30,
            )
            if not tree.is_file() or tree.stat().st_size == 0:
                raise RuntimeError("Installed EngineRoom did not render its home screen")
        finally:
            uninstaller = destination / "uninstall.exe"
            if uninstaller.is_file():
                subprocess.run([uninstaller, "/S"], check=True, timeout=120)
    print(f"Verified {installer.name}: silent install, EngineRoom home and Stockfish UCI")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installers", nargs="+", type=Path)
    for installer in parser.parse_args().installers:
        verify(installer)
