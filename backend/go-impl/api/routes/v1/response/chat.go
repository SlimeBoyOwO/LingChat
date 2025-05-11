package response

import (
	"LingChat/api"
)

type CompletionResponse struct {
	ConversationID string         `json:"conversation_id"`
	MessageID      string         `json:"message_id"`
	Messages       []api.Response `json:"messages"`
}
