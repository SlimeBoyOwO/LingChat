package data

import (
	"context"
	"errors"
	"time"

	"LingChat/internal/data/ent/ent"
	"LingChat/internal/data/ent/ent/shadow"
	"LingChat/internal/data/ent/ent/user"
)

var (
	ErrPasswordNotFound = errors.New("password not found")
)

// UserRepo 用户仓库接口
type UserRepo interface {
	// Create 创建用户及其密码记录
	Create(ctx context.Context, u *User) (*ent.User, error)
	// GetByID 通过ID获取用户
	GetByID(ctx context.Context, id int64) (*ent.User, error)
	// GetByUsername 通过用户名获取用户
	GetByUsername(ctx context.Context, username string) (*ent.User, error)
	// GetByEmail 通过邮箱获取用户
	GetByEmail(ctx context.Context, email string) (*ent.User, error)
	// Update 更新用户信息
	Update(ctx context.Context, id int64, u *User) (*ent.User, error)
	// UpdatePassword 更新用户密码
	UpdatePassword(ctx context.Context, userID int64, password string) error
	// Delete 软删除用户及其密码
	Delete(ctx context.Context, id int64) error
	// List 列出用户
	List(ctx context.Context, offset, limit int) ([]*ent.User, int, error)
	// GetPassword 获取用户密码以便在Service层验证
	GetPassword(ctx context.Context, userID int64) (string, error)
}

type User struct {
	ID       int64
	Username string
	Password string
	Email    string
}

// userRepo 用户仓库实现
type userRepo struct {
	data *Data
}

// NewUserRepo 创建用户仓库实例
func NewUserRepo(data *Data) UserRepo {
	return &userRepo{
		data: data,
	}
}

// Create 创建用户及其密码记录
func (r *userRepo) Create(ctx context.Context, u *User) (*ent.User, error) {
	// 使用事务保证用户和密码同时创建
	tx, err := r.data.db.Tx(ctx)
	if err != nil {
		return nil, err
	}

	// 回滚函数
	rollback := func(tx *ent.Tx, err error) (*ent.User, error) {
		if rerr := tx.Rollback(); rerr != nil {
			err = rerr
		}
		return nil, err
	}

	// 创建用户
	newUser, err := tx.User.
		Create().
		SetUsername(u.Username).
		SetEmail(u.Email).
		Save(ctx)
	if err != nil {
		return rollback(tx, err)
	}

	// 创建密码记录
	_, err = tx.Shadow.
		Create().
		SetPassword(u.Password).
		SetUserID(newUser.ID).
		Save(ctx)
	if err != nil {
		return rollback(tx, err)
	}

	// 提交事务
	if err := tx.Commit(); err != nil {
		return rollback(tx, err)
	}

	return newUser, nil
}

// GetByID 通过ID获取用户
func (r *userRepo) GetByID(ctx context.Context, id int64) (*ent.User, error) {
	return r.data.db.User.
		Query().
		Where(user.ID(id), user.DeletedAtIsNil()).
		Only(ctx)
}

// GetByUsername 通过用户名获取用户
func (r *userRepo) GetByUsername(ctx context.Context, username string) (*ent.User, error) {
	return r.data.db.User.
		Query().
		Where(
			user.Username(username),
			user.DeletedAtIsNil(),
		).
		Only(ctx)
}

// GetByEmail 通过邮箱获取用户
func (r *userRepo) GetByEmail(ctx context.Context, email string) (*ent.User, error) {
	return r.data.db.User.
		Query().
		Where(
			user.Email(email),
			user.DeletedAtIsNil(),
		).
		Only(ctx)
}

// GetPassword 获取用户密码以便在Service层验证
func (r *userRepo) GetPassword(ctx context.Context, userID int64) (string, error) {
	shadow, err := r.data.db.Shadow.
		Query().
		Where(shadow.UserID(userID)).
		Only(ctx)
	if err != nil {
		return "", err
	}

	return shadow.Password, nil
}

// Update 更新用户信息
func (r *userRepo) Update(ctx context.Context, id int64, u *User) (*ent.User, error) {
	update := r.data.db.User.
		UpdateOneID(id).
		SetUpdatedAt(time.Now())

	if u.Username != "" {
		update = update.SetUsername(u.Username)
	}

	if u.Email != "" {
		update = update.SetEmail(u.Email)
	}

	return update.Save(ctx)
}

// UpdatePassword 更新用户密码
func (r *userRepo) UpdatePassword(ctx context.Context, userID int64, password string) error {
	// 查找用户的密码记录
	shadow, err := r.data.db.Shadow.
		Query().
		Where(shadow.UserID(userID)).
		Only(ctx)

	if ent.IsNotFound(err) {
		// 如果没有密码记录，创建一个新的
		_, err = r.data.db.Shadow.
			Create().
			SetPassword(password).
			SetUserID(userID).
			Save(ctx)
		return err
	} else if err != nil {
		return err
	}

	// 更新密码记录
	_, err = r.data.db.Shadow.
		UpdateOne(shadow).
		SetPassword(password).
		Save(ctx)
	return err
}

// Delete 软删除用户及其密码记录
func (r *userRepo) Delete(ctx context.Context, id int64) error {
	// 使用事务保证用户和密码同时被软删除
	tx, err := r.data.db.Tx(ctx)
	if err != nil {
		return err
	}

	// 回滚函数
	rollback := func(tx *ent.Tx, err error) error {
		if rerr := tx.Rollback(); rerr != nil {
			err = rerr
		}
		return err
	}

	// 软删除密码记录
	shadow, err := tx.Shadow.
		Query().
		Where(shadow.UserID(id)).
		Only(ctx)

	if err == nil {
		// 存在密码记录，则软删除
		_, err = tx.Shadow.
			UpdateOne(shadow).
			SetDeletedAt(time.Now()).
			Save(ctx)
		if err != nil {
			return rollback(tx, err)
		}
	} else if !ent.IsNotFound(err) {
		// 查询出错，但不是未找到错误
		return rollback(tx, err)
	}
	// 如果是未找到错误，则说明没有密码记录，继续软删除用户

	// 软删除用户
	_, err = tx.User.
		UpdateOneID(id).
		SetDeletedAt(time.Now()).
		Save(ctx)
	if err != nil {
		return rollback(tx, err)
	}

	// 提交事务
	if err := tx.Commit(); err != nil {
		return rollback(tx, err)
	}

	return nil
}

// List 列出用户
func (r *userRepo) List(ctx context.Context, offset, limit int) ([]*ent.User, int, error) {
	if limit <= 0 {
		limit = 10
	}

	users, err := r.data.db.User.
		Query().
		Where(user.DeletedAtIsNil()).
		Offset(offset).
		Limit(limit).
		All(ctx)
	if err != nil {
		return nil, 0, err
	}

	// 获取总数
	total, err := r.data.db.User.
		Query().
		Where(user.DeletedAtIsNil()).
		Count(ctx)
	if err != nil {
		return nil, 0, err
	}

	return users, total, nil
}
