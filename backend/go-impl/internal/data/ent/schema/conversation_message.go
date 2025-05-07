package schema

import (
	"entgo.io/ent"
	"entgo.io/ent/schema/edge"
	"entgo.io/ent/schema/field"
	"entgo.io/ent/schema/index"
)

// ConversationMessage holds the schema definition for the ConversationMessage entity.
type ConversationMessage struct {
	ent.Schema
}

// Role enum for ConversationMessage
const (
	RoleSystem    string = "system"
	RoleUser      string = "user"
	RoleAssistant string = "assistant"
)

// Fields of the ConversationMessage.
func (ConversationMessage) Fields() []ent.Field {
	return []ent.Field{
		field.Int64("id").
			StructTag(`json:"id,omitempty"`).
			Positive().
			Immutable().
			Unique().
			Comment("The primary key"),
		field.Enum("role").
			Values(RoleSystem, RoleUser, RoleAssistant).
			Comment("The role of the message sender"),
		field.String("content").
			Comment("The content of the message"),
		field.JSON("parent_message_ids", []int{}).
			Optional().
			Comment("The IDs of parent messages"),
		field.Int64("conversation_id").
			Comment("The ID of the conversation this message belongs to"),
		field.String("status").
			Optional().
			Comment("The status of the message"),
		field.String("model").
			Optional().
			Comment("The model used to generate the message"),
		field.Int64("next_message_id").
			Optional().
			Nillable().
			Comment("The ID of the next message"),
	}
}

// Edges of the ConversationMessage.
func (ConversationMessage) Edges() []ent.Edge {
	return []ent.Edge{
		edge.From("conversation", Conversation.Type).
			Ref("messages").
			Field("conversation_id").
			Unique().
			Required().
			Comment("The conversation this message belongs to"),
		edge.To("next_messages", ConversationMessage.Type).
			From("next_message").
			Field("next_message_id").
			Unique().
			Comment("The messages following this message"),
	}
}

// Indexes of the ConversationMessage.
func (ConversationMessage) Indexes() []ent.Index {
	return []ent.Index{
		index.Fields("conversation_id"),
		index.Fields("next_message_id"),
	}
}

// Mixin of the ConversationMessage.
func (ConversationMessage) Mixin() []ent.Mixin {
	return []ent.Mixin{
		TimestampMixin{},
	}
}
