package schema

import (
	"entgo.io/ent"
	"entgo.io/ent/schema/edge"
	"entgo.io/ent/schema/field"
	"entgo.io/ent/schema/index"
)

// Conversation holds the schema definition for the Conversation entity.
type Conversation struct {
	ent.Schema
}

// Fields of the Conversation.
func (Conversation) Fields() []ent.Field {
	return []ent.Field{
		field.Int64("id").
			StructTag(`json:"id,omitempty"`).
			Positive().
			Immutable().
			Unique().
			Comment("The primary key").
			Annotations(),
		field.String("title").
			NotEmpty().
			Comment("The title of the conversation"),
		field.Int64("latest_message_id").
			Optional().
			Nillable().
			Comment("The ID of the latest message in the conversation"),
		field.Int64("user_id").
			Comment("The ID of the user who owns the conversation"),
	}
}

// Edges of the Conversation.
func (Conversation) Edges() []ent.Edge {
	return []ent.Edge{
		edge.To("messages", ConversationMessage.Type).
			Comment("The messages in the conversation"),
		edge.To("latest_message", ConversationMessage.Type).
			Field("latest_message_id").
			Unique().
			Comment("The latest message in the conversation"),
		edge.From("user", User.Type).
			Ref("conversations").
			Field("user_id").
			Unique().
			Required().
			Comment("The user who owns the conversation"),
	}
}

// Indexes of the Conversation.
func (Conversation) Indexes() []ent.Index {
	return []ent.Index{
		index.Fields("user_id"),
		index.Fields("latest_message_id"),
	}
}

// Mixin of the Conversation.
func (Conversation) Mixin() []ent.Mixin {
	return []ent.Mixin{
		TimestampMixin{},
	}
}
