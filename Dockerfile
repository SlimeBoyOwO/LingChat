FROM python:3.12

WORKDIR /app

RUN apt-get update && apt-get install -y \
    build-essential \
    python3-dev \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

RUN python -m pip install --upgrade pip && \
    pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple

COPY requirements.txt .

RUN grep -vE "PyQt|qt6|qmake|sip" requirements.txt > requirements_clean.txt && \
    cat requirements_clean.txt

RUN pip install --no-cache-dir wheel setuptools

RUN pip install --no-cache-dir -r requirements_clean.txt

COPY . .
EXPOSE 8000
EXPOSE 8765
EXPOSE 3000
COPY .env.example .env

RUN mkdir -p backend/emotion_model_18emo frontend/public/audio

CMD ["python", "./backend/windows_main.py"]