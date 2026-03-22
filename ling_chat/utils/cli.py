def print_logo():
    logo = [  #
        "█╗       ██╗ ███╗   ██╗  ██████╗      █████╗ ██╗  ██╗  █████╗  ████████╗",
        "██║      ██║ ████╗  ██║ ██╔════╝     ██╔═══╝ ██║  ██║ ██╔══██╗ ╚══██╔══╝",
        "██║      ██║ ██╔██╗ ██║ ██║  ███╗    ██║     ███████║ ███████║    ██║   ",
        "██║      ██║ ██║╚██╗██║ ██║   ██║    ██║     ██╔══██║ ██╔══██║    ██║   ",
        "███████╗ ██║ ██║ ╚████║ ╚██████╔╝     █████╗ ██║  ██║ ██║  ██║    ██║   ",
        "╚══════╝ ╚═╝ ╚═╝  ╚═══╝  ╚═════╝      ╚════╝ ╚═╝  ╚═╝ ╚═╝  ╚═╝    ╚═╝   ",  #
    ]
    for line in logo:
        print(line)


def handle_export_emotions_cli(args):
    """处理导出情感标签的CLI命令"""
    from ling_chat.utils.export_emotions import export_emotion_labels
    from ling_chat.core.logger import logger
    
    if not args.full and not args.less:
        print("请选择导出模式：")
        print("1. 全部导出模式 (--full): 导出sentence、text和label三列，包括你存档中的对话句子，可能会有隐私问题，但是数据更全面")
        print("2. 精简导出模式 (--less): 只导出text和label两列，数据简洁")
        
        while True:
            choice = input("请输入选择 (1 或 2): ").strip()
            if choice == "1":
                args.full = True
                break
            elif choice == "2":
                args.less = True
                break
            else:
                print("无效选择，请输入 1 或 2")
    
    if args.full:
        output_file, count = export_emotion_labels(args.output, full_export=True)
    else:
        output_file, count = export_emotion_labels(args.output, full_export=False)
        
    if count == 0:
        logger.warning("没有获取任何有效的情感标签数据，暂时不需要发送哦")
    else:
        logger.info(f"成功导出 {count} 条情感标签数据到：{output_file}")


def handle_install_cli(args):
    """处理安装模块的CLI命令"""
    from ling_chat.utils.runtime_path import third_party_path
    from ling_chat.third_party import install_third_party
    from ling_chat.core.logger import logger
    
    def handle_install(install_modules_list, use_mirror=False):
        for module in install_modules_list:
            logger.info(f"正在安装模块: {module}")
            if module == "vits":
                vits_path = third_party_path / "vits-simple-api/vits-simple-api-windows-cpu-v0.6.16"
                install_third_party.install_vits(vits_path)
                install_third_party.install_vits_model(vits_path)
            elif module == "sbv2":
                install_third_party.install_sbv2(third_party_path / "sbv2/sbv2")
            elif module == "18emo":
                install_third_party.install_18emo(third_party_path / "emotion_model_18emo")
            elif module == "rag":
                install_third_party.install_rag_model(use_mirror=use_mirror)
            else:
                logger.error(f"未知的安装模块: {module}")

    handle_install(args.modules, use_mirror=args.mirror)
    logger.info("安装完成")