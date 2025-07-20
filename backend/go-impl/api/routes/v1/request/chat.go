package request

type ChatCompletionRequest struct {
	Message        string `json:"message"`
	ConversationID string `json:"conversation_id"`
	PrevMessageID  string `json:"prev_message_id"`
}
