from setuptools import setup

setup(
    name="assets_py",
    version="1.0.0",
    packages=["assets_py"],
    entry_points={
        "console_scripts": [
            "uv-build=assets_py.main:main_build",
            "uv-test=assets_py.main:main_test",
        ],
    },
    python_requires=">=3.12",
)
