package schema

import (
	"entgo.io/ent"
	"entgo.io/ent/schema/edge"
	"entgo.io/ent/schema/field"
)

// Shadow holds the schema definition for the Shadow entity.
// Shadow存储用户密码，与User表分离以提高安全性
type Shadow struct {
	ent.Schema
}

// Fields of the Shadow.
func (Shadow) Fields() []ent.Field {
	return []ent.Field{
		field.Int64("id").
			Positive().
			Immutable().
			Unique(),
		field.String("password").
			Sensitive(),
		// 用户ID，通过边关系可以自动填充
		field.Int64("user_id").
			Positive().
			Unique(),
	}
}

func (Shadow) Mixin() []ent.Mixin {
	return []ent.Mixin{
		TimestampMixin{},
	}
}

// Edges of the Shadow.
func (Shadow) Edges() []ent.Edge {
	return []ent.Edge{
		edge.From("user", User.Type).
			Ref("shadow").
			Field("user_id").
			Unique().
			Required(),
	}
}
