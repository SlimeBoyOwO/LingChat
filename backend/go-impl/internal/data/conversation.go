package data

import (
	"context"
	"errors"
	"time"

	"LingChat/internal/data/ent/ent"
	"LingChat/internal/data/ent/ent/conversation"
	"LingChat/internal/data/ent/ent/conversationmessage"
)

var (
	ErrConversationNotEmpty = errors.New("conversation already has messages")
	ErrBaseMessageNotFound  = errors.New("base message not found")
	ErrConversationNotFound = errors.New("conversation not found")
)

// ConversationRepo 定义对话和消息的仓库接口
type ConversationRepo interface {
	// 对话相关操作
	CreateEmptyConversation(ctx context.Context, title string, userID int64) (*ent.Conversation, error)
	CreateConversationWithInitialMessage(ctx context.Context, title string, userID int64, role, content, model string) (*ent.Conversation, *ent.ConversationMessage, error)
	GetConversation(ctx context.Context, id int64) (*ent.Conversation, error)
	GetConversationWithMessages(ctx context.Context, id int64) (*ent.Conversation, []*ent.ConversationMessage, error)
	ListConversations(ctx context.Context, userID int64, offset, limit int) ([]*ent.Conversation, int, error)
	UpdateConversationTitle(ctx context.Context, id int64, title string) error
	DeleteConversation(ctx context.Context, id int64) error

	// 消息相关操作
	CreateHeadMessage(ctx context.Context, conversationID int64, role, content, model string) (*ent.ConversationMessage, error)
	AppendMessage(ctx context.Context, prevMessageID int64, role, content, model string) (*ent.ConversationMessage, error)
	GetMessage(ctx context.Context, id int64) (*ent.ConversationMessage, error)
	ListMessages(ctx context.Context, conversationID int64, offset, limit int) ([]*ent.ConversationMessage, int, error)
	GetMessageChain(ctx context.Context, messageID int64) ([]*ent.ConversationMessage, error)
	UpdateMessageStatus(ctx context.Context, id int64, status string) error
}

// conversationRepo 是实现 ConversationRepo 接口的仓库
type conversationRepo struct {
	data *Data
}

// NewConversationRepo 创建新的对话仓库
func NewConversationRepo(data *Data) ConversationRepo {
	return &conversationRepo{
		data: data,
	}
}

// CreateEmptyConversation 创建新的空对话
func (r *conversationRepo) CreateEmptyConversation(ctx context.Context, title string, userID int64) (*ent.Conversation, error) {
	return r.data.db.Conversation.Create().
		SetTitle(title).
		SetUserID(userID).
		Save(ctx)
}

// CreateConversationWithInitialMessage 创建新对话并添加初始消息
func (r *conversationRepo) CreateConversationWithInitialMessage(ctx context.Context, title string, userID int64, role, content, model string) (*ent.Conversation, *ent.ConversationMessage, error) {
	// 开始事务
	tx, err := r.data.db.Tx(ctx)
	if err != nil {
		return nil, nil, err
	}

	// 创建对话
	conv, err := tx.Conversation.Create().
		SetTitle(title).
		SetUserID(userID).
		Save(ctx)
	if err != nil {
		return nil, nil, tx.Rollback()
	}

	// 创建初始消息
	msgCreate := tx.ConversationMessage.Create().
		SetConversationID(conv.ID).
		SetRole(conversationmessage.Role(role)).
		SetContent(content).
		SetParentMessageIds([]int{}) // 空的父消息ID数组

	if model != "" {
		msgCreate.SetModel(model)
	}

	// 设置状态
	msgCreate.SetStatus("created")

	// 保存消息
	msg, err := msgCreate.Save(ctx)
	if err != nil {
		return nil, nil, tx.Rollback()
	}

	// 更新对话的最新消息ID
	err = tx.Conversation.UpdateOne(conv).
		SetLatestMessageID(msg.ID).
		Exec(ctx)
	if err != nil {
		return nil, nil, tx.Rollback()
	}

	// 提交事务
	if err := tx.Commit(); err != nil {
		return nil, nil, err
	}

	return conv, msg, nil
}

// GetConversation 获取单个对话
func (r *conversationRepo) GetConversation(ctx context.Context, id int64) (*ent.Conversation, error) {
	return r.data.db.Conversation.Query().
		Where(conversation.ID(id)).
		Where(conversation.DeletedAtIsNil()).
		Only(ctx)
}

// GetConversationWithMessages 获取对话及其消息
func (r *conversationRepo) GetConversationWithMessages(ctx context.Context, id int64) (*ent.Conversation, []*ent.ConversationMessage, error) {
	conv, err := r.data.db.Conversation.Query().
		Where(conversation.ID(id)).
		Where(conversation.DeletedAtIsNil()).
		Only(ctx)
	if err != nil {
		return nil, nil, err
	}

	msgs, err := r.data.db.ConversationMessage.Query().
		Where(conversationmessage.ConversationID(id)).
		Where(conversationmessage.DeletedAtIsNil()).
		Order(ent.Asc(conversationmessage.FieldCreatedAt)).
		All(ctx)
	if err != nil {
		return nil, nil, err
	}

	return conv, msgs, nil
}

// ListConversations 列出用户的对话
func (r *conversationRepo) ListConversations(ctx context.Context, userID int64, offset, limit int) ([]*ent.Conversation, int, error) {
	// 查询总数
	count, err := r.data.db.Conversation.Query().
		Where(conversation.UserID(userID)).
		Where(conversation.DeletedAtIsNil()).
		Count(ctx)
	if err != nil {
		return nil, 0, err
	}

	// 分页查询
	convs, err := r.data.db.Conversation.Query().
		Where(conversation.UserID(userID)).
		Where(conversation.DeletedAtIsNil()).
		Order(ent.Desc(conversation.FieldUpdatedAt)).
		Offset(offset).
		Limit(limit).
		All(ctx)
	if err != nil {
		return nil, 0, err
	}

	return convs, count, nil
}

// UpdateConversationTitle 更新对话标题
func (r *conversationRepo) UpdateConversationTitle(ctx context.Context, id int64, title string) error {
	return r.data.db.Conversation.UpdateOneID(id).
		SetTitle(title).
		Exec(ctx)
}

// DeleteConversation 删除对话（软删除）
func (r *conversationRepo) DeleteConversation(ctx context.Context, id int64) error {
	return r.data.db.Conversation.UpdateOneID(id).
		SetDeletedAt(time.Now()).
		Exec(ctx)
}

// CreateHeadMessage 为空对话创建第一条消息
func (r *conversationRepo) CreateHeadMessage(ctx context.Context, conversationID int64, role, content, model string) (*ent.ConversationMessage, error) {
	// 检查对话是否存在
	conv, err := r.GetConversation(ctx, conversationID)
	if err != nil {
		return nil, ErrConversationNotFound
	}

	// 检查对话是否已有消息
	count, err := r.data.db.ConversationMessage.Query().
		Where(conversationmessage.ConversationID(conversationID)).
		Where(conversationmessage.DeletedAtIsNil()).
		Count(ctx)
	if err != nil {
		return nil, err
	}

	if count > 0 {
		return nil, ErrConversationNotEmpty
	}

	// 开始事务
	tx, err := r.data.db.Tx(ctx)
	if err != nil {
		return nil, err
	}

	// 创建头消息
	msgCreate := tx.ConversationMessage.Create().
		SetConversationID(conversationID).
		SetRole(conversationmessage.Role(role)).
		SetContent(content).
		SetParentMessageIds([]int{}) // 空的父消息ID数组

	if model != "" {
		msgCreate.SetModel(model)
	}

	// 设置状态
	msgCreate.SetStatus("created")

	// 保存消息
	msg, err := msgCreate.Save(ctx)
	if err != nil {
		return nil, tx.Rollback()
	}

	// 更新对话的最新消息ID
	err = tx.Conversation.UpdateOne(conv).
		SetLatestMessageID(msg.ID).
		Exec(ctx)
	if err != nil {
		return nil, tx.Rollback()
	}

	// 提交事务
	if err := tx.Commit(); err != nil {
		return nil, err
	}

	return msg, nil
}

// AppendMessage 追加消息到已有消息后面
func (r *conversationRepo) AppendMessage(ctx context.Context, prevMessageID int64, role, content, model string) (*ent.ConversationMessage, error) {
	// 查找前一条消息
	prevMsg, err := r.GetMessage(ctx, prevMessageID)
	if err != nil {
		return nil, ErrBaseMessageNotFound
	}

	// 开始事务
	tx, err := r.data.db.Tx(ctx)
	if err != nil {
		return nil, err
	}

	// 创建新消息
	msgCreate := tx.ConversationMessage.Create().
		SetConversationID(prevMsg.ConversationID).
		SetRole(conversationmessage.Role(role)).
		SetContent(content).
		SetParentMessageIds(append(prevMsg.ParentMessageIds, int(prevMsg.ID))) // 继承前一条消息的父消息列表并添加前一条消息ID

	if model != "" {
		msgCreate.SetModel(model)
	}

	// 设置状态
	msgCreate.SetStatus("created")

	// 保存消息
	msg, err := msgCreate.Save(ctx)
	if err != nil {
		return nil, tx.Rollback()
	}

	// 更新前一条消息，设置其nextMessageID为新消息的ID
	err = tx.ConversationMessage.UpdateOne(prevMsg).
		SetNextMessageID(msg.ID).
		Exec(ctx)
	if err != nil {
		return nil, tx.Rollback()
	}

	// 更新对话的最新消息ID
	err = tx.Conversation.UpdateOneID(prevMsg.ConversationID).
		SetLatestMessageID(msg.ID).
		Exec(ctx)
	if err != nil {
		return nil, tx.Rollback()
	}

	// 提交事务
	if err := tx.Commit(); err != nil {
		return nil, err
	}

	return msg, nil
}

// GetMessage 获取单个消息
func (r *conversationRepo) GetMessage(ctx context.Context, id int64) (*ent.ConversationMessage, error) {
	return r.data.db.ConversationMessage.Query().
		Where(conversationmessage.ID(id)).
		Where(conversationmessage.DeletedAtIsNil()).
		Only(ctx)
}

// ListMessages 列出对话的消息
func (r *conversationRepo) ListMessages(ctx context.Context, conversationID int64, offset, limit int) ([]*ent.ConversationMessage, int, error) {
	// 查询总数
	count, err := r.data.db.ConversationMessage.Query().
		Where(conversationmessage.ConversationID(conversationID)).
		Where(conversationmessage.DeletedAtIsNil()).
		Count(ctx)
	if err != nil {
		return nil, 0, err
	}

	// 分页查询
	msgs, err := r.data.db.ConversationMessage.Query().
		Where(conversationmessage.ConversationID(conversationID)).
		Where(conversationmessage.DeletedAtIsNil()).
		Order(ent.Asc(conversationmessage.FieldCreatedAt)).
		Offset(offset).
		Limit(limit).
		All(ctx)
	if err != nil {
		return nil, 0, err
	}

	return msgs, count, nil
}

// GetMessageChain 获取消息链（从给定消息追溯到最初消息）
func (r *conversationRepo) GetMessageChain(ctx context.Context, messageID int64) ([]*ent.ConversationMessage, error) {
	// 获取起始消息
	currentMsg, err := r.GetMessage(ctx, messageID)
	if err != nil {
		return nil, err
	}

	// 如果没有父消息，直接返回当前消息
	if len(currentMsg.ParentMessageIds) == 0 {
		return []*ent.ConversationMessage{currentMsg}, nil
	}

	// 获取所有父消息ID（已按照时间顺序排列）
	parentIDs := currentMsg.ParentMessageIds
	var messageIDs []int64
	for _, id := range parentIDs {
		messageIDs = append(messageIDs, int64(id))
	}

	// 添加当前消息ID
	messageIDs = append(messageIDs, currentMsg.ID)

	// 批量查询所有消息
	messages, err := r.data.db.ConversationMessage.Query().
		Where(conversationmessage.IDIn(messageIDs...)).
		Where(conversationmessage.DeletedAtIsNil()).
		All(ctx)
	if err != nil {
		return nil, err
	}

	// 将消息按照parentMessageIds的顺序排列
	orderedMessages := make([]*ent.ConversationMessage, len(messageIDs))
	messageMap := make(map[int64]*ent.ConversationMessage)

	// 建立消息ID到消息的映射
	for _, msg := range messages {
		messageMap[msg.ID] = msg
	}

	// 按照ID顺序填充结果数组
	for i, id := range messageIDs {
		if msg, ok := messageMap[id]; ok {
			orderedMessages[i] = msg
		} else {
			// 如果找不到某条消息，跳过它
			return nil, errors.New("incomplete message chain: missing message")
		}
	}

	return orderedMessages, nil
}

// UpdateMessageStatus 更新消息状态
func (r *conversationRepo) UpdateMessageStatus(ctx context.Context, id int64, status string) error {
	return r.data.db.ConversationMessage.UpdateOneID(id).
		SetStatus(status).
		Exec(ctx)
}
