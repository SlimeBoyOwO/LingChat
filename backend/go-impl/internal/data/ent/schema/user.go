package schema

import (
	"entgo.io/ent"
	"entgo.io/ent/schema/field"
	"entgo.io/ent/schema/index"
	// "entgo.io/ent/schema/edge" // 如果需要关联其他表，可以取消注释
)

// User holds the schema definition for the User entity.
type User struct {
	ent.Schema
}

// Fields of the User.
func (User) Fields() []ent.Field {
	return []ent.Field{
		field.Int64("id").
			Positive().
			Immutable().
			Unique(),
		field.String("username").
			NotEmpty().
			Unique().
			MaxLen(64),
		field.String("shadow").
			NotEmpty().
			Sensitive(),
		field.String("email").
			Optional().
			Unique().
			MaxLen(100),
	}
}

func (User) Mixin() []ent.Mixin {
	return []ent.Mixin{
		TimestampMixin{},
	}
}

// Edges of the User.
func (User) Edges() []ent.Edge {
	return []ent.Edge{}
}

// Indexes of the User.
func (User) Indexes() []ent.Index {
	return []ent.Index{
		index.Fields("username"),
		index.Fields("email"),
	}
}
