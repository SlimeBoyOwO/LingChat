package service

import (
	"context"
	"errors"
	"fmt"
	"strconv"

	"github.com/sashabaranov/go-openai"

	"LingChat/api/routes/common"
	"LingChat/internal/data"
	"LingChat/internal/data/ent/ent"
	"LingChat/internal/data/ent/ent/conversationmessage"
)

// ConversationService 处理与对话相关的业务逻辑
type ConversationService struct {
	conversationRepo      data.ConversationRepo
	legacyTempChatContext *data.LegacyTempChatContext
	configModel           string
}

// NewConversationService 创建一个新的 ConversationService 实例
func NewConversationService(
	conversationRepo data.ConversationRepo,
	legacyTempChatContext *data.LegacyTempChatContext,
	configModel string,
) *ConversationService {
	return &ConversationService{
		conversationRepo:      conversationRepo,
		legacyTempChatContext: legacyTempChatContext,
		configModel:           configModel,
	}
}

// RecordConversationAndMessage 处理会话和消息存储逻辑
func (s *ConversationService) RecordConversationAndMessage(ctx context.Context, message string, conversationID, prevMessageID string) (*ent.Conversation, *ent.ConversationMessage, error) {
	var userMsgObj *ent.ConversationMessage
	var conv *ent.Conversation

	user := common.GetUserFromContext(ctx)
	if user == nil {
		user = &ent.User{}
	}

	// 创建标题，限制长度为20个字符
	title := message
	if len(title) > 20 {
		title = title[:20] + "..."
	}

	// 将会话存储到数据库中
	switch {
	case conversationID == "" && prevMessageID == "":
		// 两者都为空，创建新的对话和消息
		var userMsgObjs []*ent.ConversationMessage
		var err error
		conv, userMsgObjs, err = s.conversationRepo.CreateConversationWithMessages(
			ctx,
			title, // 使用用户消息的前20个字符作为标题
			user.ID,
			data.MessageInput{
				Role:    string(conversationmessage.RoleSystem),
				Content: data.SystemPrompt,
			},
			data.MessageInput{
				Role:    string(conversationmessage.RoleUser),
				Content: message,
			},
		)
		if err != nil {
			return nil, nil, fmt.Errorf("创建对话失败: %w", err)
		}
		userMsgObj = userMsgObjs[len(userMsgObjs)-1]

	case conversationID != "" && prevMessageID == "":
		// 有对话ID但没有前一条消息ID，在对话末尾添加
		convID, err := strconv.ParseInt(conversationID, 10, 64)
		if err != nil {
			return nil, nil, fmt.Errorf("无效的对话ID: %w", err)
		}

		// 获取对话
		var getErr error
		conv, getErr = s.conversationRepo.GetConversation(ctx, convID)
		if getErr != nil {
			return nil, nil, fmt.Errorf("获取对话失败: %w", getErr)
		}

		// 检查权限：会话所属用户必须与当前用户一致，或者会话所属用户ID为0（表示可以被所有人使用）
		if conv.UserID != 0 && conv.UserID != user.ID {
			return nil, nil, fmt.Errorf("无权访问此对话")
		}

		userMsgObj, err = s.conversationRepo.AppendMessageToConversation(
			ctx,
			convID,
			string(conversationmessage.RoleUser),
			message,
			"",
		)
		if err != nil {
			return nil, nil, fmt.Errorf("添加消息到对话失败: %w", err)
		}

	default:
		// 有前一条消息ID，直接在其后追加，无视是否传入对话ID
		prevMsgID, err := strconv.ParseInt(prevMessageID, 10, 64)
		if err != nil {
			return nil, nil, fmt.Errorf("无效的消息ID: %w", err)
		}

		// 获取消息
		prevMsg, getErr := s.conversationRepo.GetMessage(ctx, prevMsgID)
		if getErr != nil {
			return nil, nil, fmt.Errorf("获取前一条消息失败: %w", getErr)
		}

		// 获取对话
		conv, getErr = s.conversationRepo.GetConversation(ctx, prevMsg.ConversationID)
		if getErr != nil {
			return nil, nil, fmt.Errorf("获取对话失败: %w", getErr)
		}

		// 检查权限：会话所属用户必须与当前用户一致，或者会话所属用户ID为0（表示可以被所有人使用）
		if conv.UserID != 0 && conv.UserID != user.ID {
			return nil, nil, fmt.Errorf("无权访问此对话")
		}

		userMsgObj, err = s.conversationRepo.AppendMessage(
			ctx,
			prevMsgID,
			string(conversationmessage.RoleUser),
			message,
			"",
		)
		if err != nil {
			return nil, nil, fmt.Errorf("追加消息失败: %w", err)
		}
	}

	if userMsgObj == nil {
		return nil, nil, errors.New("无法处理聊天历史")
	}

	return conv, userMsgObj, nil
}

// GetChatContext 获取消息链
func (s *ConversationService) GetChatContext(ctx context.Context, messageID int64) ([]openai.ChatCompletionMessage, error) {
	entMsgs, err := s.conversationRepo.GetMessageChain(ctx, messageID)
	if err != nil {
		return nil, err
	}

	messages := make([]openai.ChatCompletionMessage, 0)
	for _, entMsg := range entMsgs {
		messages = append(messages, openai.ChatCompletionMessage{
			Role:    string(entMsg.Role),
			Content: entMsg.Content,
		})
	}

	return messages, nil
}

// SaveAssistantMessage 将助手回复保存到数据库
func (s *ConversationService) SaveAssistantMessage(ctx context.Context, prevMessageID int64, content string) (*ent.ConversationMessage, error) {
	assistantMsg, err := s.conversationRepo.AppendMessage(
		ctx,
		prevMessageID,
		string(conversationmessage.RoleAssistant),
		content,
		s.configModel,
	)
	if err != nil {
		return nil, fmt.Errorf("保存助手回复失败: %w", err)
	}
	return assistantMsg, nil
}

// GetChatHistory 获取聊天历史
func (s *ConversationService) GetChatHistory(ctx context.Context) []openai.ChatCompletionMessage {
	return s.legacyTempChatContext.DumpMessage()
}

// LoadChatHistory 加载聊天历史
func (s *ConversationService) LoadChatHistory(ctx context.Context, msg []openai.ChatCompletionMessage) []openai.ChatCompletionMessage {
	return s.legacyTempChatContext.LoadMessage(msg)
}
