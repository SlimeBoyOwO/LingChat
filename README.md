

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

## Project Structure
```
py_ling_chat
├── py_ling_chat  # Main package directory
│   ├── __init__.py
│   ├── api  # API-related code
│   ├── core  # Core functionality
│   ├── database  # Database-related code
│   ├── static
│   │   ├── frontend  # Frontend files
│   │   └── game_data  # Game data files
│   ├── third_party  # Third-party integrations
│   │   ├── emotion_model_18emo  # Emotion model for 18 emotion
│   │   └── vits-simple-api  # VITS Simple API for text-to-speech
│   ├── utils  # Utility functions
│   ├── __init__.py
│   ├── __main__.py
│   └── main.py  # Main entry point
├── data  # User Data files
├── docs  # Documentation files
├── tests  # Test files
├── .env  # Environment variables file (user should create this)
├── .env.example  # Example environment variables file
├── .gitignore  # Git ignore file
├── README.md  # Project README file
└── pyproject.toml  # Poetry configuration file
```
