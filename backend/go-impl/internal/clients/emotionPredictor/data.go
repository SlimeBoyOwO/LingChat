package emotionPredictor

// PredictionResponse represents the structure of the API response
type PredictionResponse struct {
	Label      string     `json:"label"`
	Confidence float64    `json:"confidence"`
	Top3       []TopLabel `json:"top3"`
	Warning    *string    `json:"warning"`
}

// TopLabel represents each entry in the top3 array
type TopLabel struct {
	Label       string  `json:"label"`
	Probability float64 `json:"probability"`
}
