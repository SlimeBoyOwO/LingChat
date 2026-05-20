// 用 DeepSeek 优化代码及补充注释，用 GCC/G++ 编译！！！用 GCC/G++ 编译！！！用 GCC/G++ 编译！！！
#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include <stdlib.h>
#include <cpuid.h>

typedef enum
{
    PERF_INTERNET,
    PERF_LOW,
    PERF_MEDIUM,
    PERF_HIGH
} PerfTier;

// 获取 CPU 品牌字符串（需要至少 48 字节缓冲区）
static int get_brand_string(char *brand, size_t size)
{
    unsigned int eax, ebx, ecx, edx;
    unsigned int max_ext;

    // 查询扩展功能最大叶号
    if (!__get_cpuid(0x80000000, &max_ext, &ebx, &ecx, &edx))
        return 0;
    if (max_ext < 0x80000004) // 品牌字符串至少需要 0x80000002~0x80000004
        return 0;

    char *ptr = brand;
    memset(brand, 0, size);

    // 依次读取三个叶子的 16 字节（合计 48 字节）
    for (unsigned int leaf = 0x80000002; leaf <= 0x80000004; leaf++)
    {
        if (!__get_cpuid(leaf, &eax, &ebx, &ecx, &edx))
            break;
        memcpy(ptr, &eax, 4);
        ptr += 4;
        memcpy(ptr, &ebx, 4);
        ptr += 4;
        memcpy(ptr, &ecx, 4);
        ptr += 4;
        memcpy(ptr, &edx, 4);
        ptr += 4;
    }

    // 去除前导空格
    char *first_non_space = brand;
    while (*first_non_space == ' ')
        first_non_space++;
    if (first_non_space != brand)
        memmove(brand, first_non_space, strlen(first_non_space) + 1);
    return 1;
}

// 获取 CPU Family 与 Model（用于代数判断）
static void get_family_model(unsigned int *family, unsigned int *model)
{
    unsigned int eax, ebx, ecx, edx;
    if (!__get_cpuid(1, &eax, &ebx, &ecx, &edx))
    {
        *family = *model = 0;
        return;
    }

    unsigned int base_family = (eax >> 8) & 0xF;
    unsigned int base_model = (eax >> 4) & 0xF;
    unsigned int ext_family = (eax >> 20) & 0xFF;
    unsigned int ext_model = (eax >> 16) & 0xF;

    if (base_family == 0xF)
        *family = base_family + ext_family;
    else
        *family = base_family;

    if (base_family == 0x6 || base_family == 0xF)
        *model = (ext_model << 4) | base_model;
    else
        *model = base_model;
}

// 从品牌字符串提取 Core i代数（例如 "i7-8550U" -> 8，"i5-12600K" -> 12）
static int extract_core_generation(const char *brand)
{
    const char *p = strstr(brand, "Core");
    if (!p)
        return -1;

    const char *i_marker = NULL;
    const char *patterns[] = {"i3-", "i5-", "i7-", "i9-", "m3-", "m5-", "m7-"}; // 扩展支持Intel Core m系列
    for (int idx = 0; idx < 7; idx++)
    {
        i_marker = strstr(p, patterns[idx]);
        if (i_marker)
            break;
    }
    if (!i_marker)
        return -1;

    i_marker += 3; // 跳过 "iX-"和"mX-"，指向数字起始位置

    // 解析连续数字串
    char *endptr;
    long model_num = strtol(i_marker, &endptr, 10);
    if (endptr == i_marker)
        return -1; // 无数字

    // 根据 Intel 命名规则推算代数：
    // - 若数值 >= 1000，代数为 model_num / 1000（整数除法），改进指针乱飞的问题
    // - 若数值 < 1000，是初代酷睿，返回-1
    if (model_num >= 1000)
        return (int)(model_num / 1000);
    else
        return -1;
}

// 检查是否为低压 / 低功耗后缀（例如U、Y、G、P 等）
static int is_low_power_suffix(const char *brand)
{
    // 定位 "CPU @" 之前的部分
    const char *cpu_at = strstr(brand, "CPU @");
    if (!cpu_at)
        return 0;

    // 向前寻找型号的末尾
    const char *end = cpu_at;
    while (end > brand && *(end - 1) != ' ')
        end--;

    // 提取从型号起始（通常为 'i' 或 'C' 等）到 end 之间的字符串
    size_t len = cpu_at - end;
    if (len == 0)
        return 0;

    // 栈上临时缓冲区拷贝
    char temp[64];
    if (len >= sizeof(temp))
        len = sizeof(temp) - 1;
    memcpy(temp, end, len);
    temp[len] = '\0';

    // 特征后缀：U（低压移动版）、Y（极低功耗）,可根据需要继续扩展。
    // H（标压移动版）、G（含集成显卡的低压版本，包括NUC）、P（中等功耗但相对标压低一些）不算低压。
    const char *suffix_list[] = {"U", "Y"};
    for (size_t i = 0; i < sizeof(suffix_list) / sizeof(suffix_list[0]); i++)
    {
        if (strstr(temp, suffix_list[i]) != NULL)
            return 1;
    }
    return 0;
}

// 主分级函数
PerfTier classify_cpu_performance(void)
{
    // 1. 检查是否为 Intel 厂商
    unsigned int eax, ebx, ecx, edx;
    if (!__get_cpuid(0, &eax, &ebx, &ecx, &edx))
        return PERF_LOW;

    // 这一串魔数组合起来是GenuineIntel
    if (ebx != 0x756e6547 || edx != 0x49656e69 || ecx != 0x6c65746e)
    {
        /* * TODO(Heiyaha): AMD Performance Classification
         * Note: AMD's naming convention is a chaotic masterpiece designed to confuse
         * both humans and static analysis tools.
         * If you are an AMD naming engineer or a "Ryzen Yes" fanboy,
         * please submit a PR to fix this mess. Thanks a lot. (nya~)
         */

        return PERF_LOW; // 非 Intel CPU 滚到 Low
    }

    // 2. 获取品牌字符串
    char brand[49];
    if (!get_brand_string(brand, sizeof(brand)))
        return PERF_LOW;

    // 3. 获取 Family/Model（备用，可扩展其他判断）
    unsigned int family, model;
    get_family_model(&family, &model);
    (void)family;
    (void)model; // 暂时未用（避免编译警告）

    // 4. 根据品牌字符串分类
    if (strstr(brand, "Atom") != NULL)
    {
        return PERF_INTERNET;
    }

    if (strstr(brand, "Celeron") != NULL || strstr(brand, "Pentium") != NULL)
    {
        // 低压后缀就Internet，否则 LOW
        return is_low_power_suffix(brand) ? PERF_INTERNET : PERF_LOW;
    }

    if (strstr(brand, "Core") != NULL)
    {
        int gen = extract_core_generation(brand);
        int low_power = is_low_power_suffix(brand);

        // 代数 >= 8 一律 HIGH
        if (gen >= 8)
        {
            return PERF_HIGH;
        }

        if (strstr(brand, "12th Gen") != NULL)
        {
            return PERF_HIGH; // 特判12代及以上都是 HIGH（mlgb，i5-1240P害死我了）
        }

        // Core Ultra 系列，我没有TAT
        if (strstr(brand, "Ultra") != NULL)
        {
            return PERF_HIGH;
        }

        if (gen > 0 && gen < 8)
        {
            // 判断是否为 i3
            int is_i3 = (strstr(brand, "i3-") != NULL);

            // - i3 且代数 <5 LOW
            if (is_i3 && gen < 5)
                return PERF_LOW;

            // - 低压 Core i，代数 <=7 时归为 LOW
            if (low_power && gen <= 7)
                return PERF_LOW;

            // 其余桌面/标压 7 代前 MEDIUM
            return PERF_MEDIUM;
        }

        // Core 2 系列（超5GHz 再战5年）
        if (strstr(brand, "Duo") || strstr(brand, "Quad") || strstr(brand, "Extreme"))
        {
            return PERF_LOW;
        }

        // 低电压CPU
        if (low_power)
        {
            return PERF_LOW;
        }
    }

    // Xeon 洋垃圾 （E3/E5/E7）
    if (strstr(brand, "Xeon") != NULL)
    {
        if (strstr(brand, "E5") || strstr(brand, "E7"))
            return PERF_HIGH;

        return PERF_MEDIUM;
    }

    // 非 Intel 处理器，默认为 LOW
    return PERF_LOW;
}

int main(void)
{
    FILE *fp;
    
    fp = freopen("lingchat_perf.ini", "r", stdin);
    if (fp != NULL)
    {
        return 0;
    }

    freopen("lingchat_perf.ini", "w", stdout);
    
    char brand[49] = "";
    int has_brand = get_brand_string(brand, sizeof(brand));

    PerfTier tier = classify_cpu_performance();
    const char *tier_names[] = {"INTERNET", "LOW", "MEDIUM", "HIGH"};
    printf("CPU_PERF=%s\n", tier_names[tier]);

    printf("%s\n", brand);

    return 0;
}
