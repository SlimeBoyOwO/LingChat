

## Installation
### Installation via pip
you may want to install the package in a virtual environment to avoid conflicts with other packages. You can create a virtual environment using `venv` or `virtualenv`.
```bash
python -m venv .env
```
then activate the virtual environment:
```bash
source .env/bin/activate  # On Linux or macOS
.env\Scripts\activate  # On Windows
```
you can install the package using pip:
```bash
pip install .
```

### Installation via poetry
you can install the package using poetry:
```bash
poetry install
```

## Usage

### add your key
create a file named `.env` in the root directory of the project and add your Chat API key to it:
```text
CHAT_API_KEY="sk-<here-is-your-key>"
```

### run the server
run the following command to start the server:
```bash
# if you installed the package via pip
# python -m py_ling_chat
# or: 
poetry run python -m py_ling_chat
```
