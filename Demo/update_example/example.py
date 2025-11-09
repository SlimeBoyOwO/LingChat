from update_main import create_application
import time
def main():
    app = create_application(version_file="version", update_url="http://localhost:5100/updates")
    um = app.update_manager

    print("当前版本:", app.version)
    try:
        while True:
            print("\n1. 检查更新")
            print("2. 下载并应用更新（需先检查并发现更新）")
            print("3. 回滚到上次备份")
            print("4. 退出")
            choice = input("请选择操作: ").strip()

            if choice == "1":
                try:
                    found = app.manual_check_update()
                    if found:
                        info = um.get_update_info() or {}
                        print(f"发现更新: {info.get('version')}")
                        print(f"更新内容：{info.get('changelog','无')}")
                    else:
                        print("当前已是最新版本")
                except Exception as e:
                    print("检查更新失败:", e)

            elif choice == "2":
                if not um.is_update_available():
                    print("没有可用更新，请先检查更新")
                    continue
                # 询问是否备份
                b = input("是否在应用前创建全量备份? (y/n): ").strip().lower()
                do_backup = b == "y"
                print("开始下载...")
                try:
                    ok = app.start_update(backup=do_backup)
                    if ok:
                        print("更新完成，请重启应用以生效。")
                    else:
                        print("更新失败，可选择回滚或查看日志。")
                except Exception as e:
                    print("更新过程中发生异常:", e)

            elif choice == "3":
                c = input("确认回滚到上次备份吗？(y/n): ").strip().lower()
                if c != "y":
                    continue
                try:
                    ok = app.rollback()
                    if ok:
                        print("回滚成功，请重启应用。")
                    else:
                        print("回滚失败或没有可用备份。")
                except Exception as e:
                    print("回滚异常:", e)

            elif choice == "4":
                break
            else:
                print("无效选择")

            # 小间隔，避免紧密循环
            time.sleep(0.2)

    except KeyboardInterrupt:
        print("\n用户中断，退出。")
    finally:
        um.stop()

if __name__ == "__main__":
    main()