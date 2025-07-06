package data

import (
	"context"
	"log"

	entsql "entgo.io/ent/dialect/sql"
	"github.com/redis/go-redis/v9"

	_ "github.com/go-sql-driver/mysql"

	"LingChat/internal/data/ent/ent"
	"LingChat/internal/data/ent/ent/migrate"
)

type Data struct {
	db    *ent.Client
	redis *redis.Client
}

func NewData(entClient *ent.Client, redisClient *redis.Client) (*Data, func(), error) {

	d := &Data{
		db:    entClient,
		redis: redisClient,
	}

	cleanup := func() {
		log.Println("closing the data resources")
		if err := d.db; err != nil {
			log.Fatal(err)
		}
	}
	return d, cleanup, nil
}

func NewEntClient(ctx context.Context, dialect string, source string, AutoMigrate bool) (*ent.Client, error) {

	drv, err := entsql.Open(dialect, source)
	if err != nil {
		return nil, err
	}
	// db := drv.DB()
	// db.SetMaxIdleConns(c.MaxIdle)
	// db.SetMaxOpenConns(c.MaxActive)
	// db.SetConnMaxLifetime(time.Duration(c.MaxLifetime) * time.Second)

	client := ent.NewClient(ent.Driver(drv))
	if AutoMigrate {
		if err := client.Schema.Create(ctx, migrate.WithForeignKeys(false)); err != nil {
			return nil, err
		}
	}
	return client, nil
}

// func NewRedisClient(ctx context.Context, conf *configs.Config) (*redis.Client, error) {
// 	rdb := redis.NewClient(&redis.Options{
// 		Addr:     conf.Redis.Addr,
// 		Password: conf.Redis.Password,
// 		DB:       conf.Redis.DB,
// 	})
// 	_, err := rdb.Ping(ctx).Result()
// 	if err != nil {
// 		return nil, sidererr.WithCaller(err)
// 	}
// 	return rdb, nil
// }
