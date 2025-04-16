from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import List, Dict, Optional
import uvicorn
from predictor import EmotionClassifier
import os

app = FastAPI(
    title="Emotion Classification API",
    description="API for emotion classification using BERT model",
    version="1.0.0"
)

# 初始化分类器
classifier = None

class PredictionRequest(BaseModel):
    text: str
    confidence_threshold: Optional[float] = 0.08

class EmotionResult(BaseModel):
    label: str
    probability: float

class PredictionResponse(BaseModel):
    label: str
    confidence: float
    top3: List[EmotionResult]
    warning: Optional[str] = None

@app.on_event("startup")
async def startup_event():
    global classifier
    try:
        model_path = os.environ.get("EMOTION_MODEL_PATH", "./emotion_model_12emo")
        classifier = EmotionClassifier(model_path)
    except Exception as e:
        raise Exception(f"Failed to initialize classifier: {str(e)}")

@app.get("/health")
async def health_check():
    return {"status": "healthy"}

@app.post("/predict", response_model=PredictionResponse)
async def predict_emotion(request: PredictionRequest):
    if classifier is None:
        raise HTTPException(status_code=500, detail="Classifier not initialized")
    
    try:
        result = classifier.predict(request.text, request.confidence_threshold)
        return result
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    uvicorn.run(
        "predictor_server:app",
        host="0.0.0.0",
        port=8000,
        workers=1,
        log_level="info"
    ) 