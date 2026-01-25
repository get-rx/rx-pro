"""
T-Rex - A blazing-fast Python package manager written in Rust

This module provides Python bindings to the T-Rex package manager,
allowing you to use its fast Rust implementation directly from Python.

Example:
    >>> from trex import resolve, sync, build, audit
    >>>
    >>> # Resolve dependencies
    >>> packages = resolve(["requests>=2.28", "numpy"])
    >>> for name, version, url in packages:
    ...     print(f"{name}=={version}")
    >>>
    >>> # Sync project
    >>> count = sync("./my-project")
    >>> print(f"Installed {count} packages")
    >>>
    >>> # Build wheel
    >>> result = build("./my-project")
    >>> print(f"Built: {result['wheel']}")
    >>>
    >>> # Security audit
    >>> vulns = audit("./my-project")
    >>> for pkg, ver, cve, sev, desc in vulns:
    ...     print(f"{pkg}=={ver}: {cve} ({sev})")
"""

from trex._trex import resolve, sync, build, audit, version

__version__ = version()
__all__ = ["resolve", "sync", "build", "audit", "version", "__version__"]


def _cli_main():
    """CLI entry point - delegates to the Rust binary."""
    import subprocess
    import sys
    import shutil

    # Try to find the rx binary
    rx_path = shutil.which("rx")
    if rx_path:
        sys.exit(subprocess.call([rx_path] + sys.argv[1:]))
    else:
        print("Error: rx binary not found. Please install T-Rex:", file=sys.stderr)
        print("  pip install t-rex", file=sys.stderr)
        sys.exit(1)
