#include <iostream>
#include <string>

int main() {
    std::string input;
    while (std::getline(std::cin, input)) {  // 从 stdin 读取输入
        if (input == "你好") {
            std::cout << "{\"reply\":\"你也好呀\",\"audioFile\":\"hit.wav\"}" << std::endl;
        }
        else {
            std::cout << "{\"reply\":\"我收到了: " << input << "\"}" << std::endl;
        }
    }
    return 0;
}