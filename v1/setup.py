from setuptools import setup, find_packages

setup(
    name="termcall",
    version="0.2.0",
    packages=["termcall"],
    install_requires=[
        "requests",
        "aiortc>=1.9.0",
        "prompt_toolkit>=3.0.47",
        "sixel>=0.1.2",
        "opencv-python>=4.10.0",
        "image-to-ascii>=0.2.2",
        "pynput>=1.7.7",
    ],
    entry_points={
        "console_scripts": [
            "termcall = termcall.main:cli_main",
        ],
    },
    author="Shrey Joshi",
    author_email="shreyjoshi2004@gmail.com",
    description="A CLI-based video/audio calling app using WebRTC and Firebase",
    long_description=open("README.md").read(),
    long_description_content_type="text/markdown",
    url="https://github.com/ShreyJ1729/termcall",
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
    ],
    python_requires=">=3.8",
)
