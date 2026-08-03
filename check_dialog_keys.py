import re
files = ['src/locales/zh-CN/settings.ts', 'src/locales/zh-HK/settings.ts', 'src/locales/ja/settings.ts', 'src/locales/en/settings.ts']
for f in files:
    with open(f, 'r', encoding='utf-8') as fp:
        content = fp.read()
    # 找 dialog: { 开始
    start = content.find('dialog: {')
    if start < 0:
        print(f, '-> NO dialog section')
        continue
    # 找匹配的结束 }
    depth = 0
    i = start + len('dialog: {')
    end = -1
    while i < len(content):
        c = content[i]
        if c == '{':
            depth += 1
        elif c == '}':
            if depth == 0:
                end = i
                break
            depth -= 1
        i += 1
    if end < 0:
        print(f, '-> UNCLOSED')
        continue
    body = content[start:end+1]
    keys = re.findall(r'\b(\w+):\s*[\'"]', body)
    unique = sorted(set(keys))
    print(f, '->', len(unique), 'keys')
    for k in ['title', 'description', 'backgroundImage', 'noImage', 'upload', 'change', 'clear', 'sizeHint', 'opacity', 'blur', 'borderRadius', 'gradientColor', 'textColor', 'resetDefault', 'resetGradientTitle', 'resetTextTitle', 'preview', 'previewName', 'previewPlaceholder', 'resetAll', 'imageTooLarge', 'unsupportedFormat', 'readFailed', 'interaction', 'scrollHistory', 'spacebarHide', 'autoHideOnThink', 'noHistory']:
        status = 'OK' if k in unique else 'MISSING'
        print(f'  {k}: {status}')
