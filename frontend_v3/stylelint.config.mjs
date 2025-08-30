/** @type {import('stylelint').Config} */
export default {
    extends: ["stylelint-config-standard"],
    plugins: ["stylelint-order"],
    rules: {
        // 嵌套选择器规则
        "selector-nested-pattern": "^&|&$",
        "no-descending-specificity": null,

        // No Duplicate
        "declaration-block-no-duplicate-custom-properties": true,
        "declaration-block-no-duplicate-properties": true,
        "font-family-no-duplicate-names": true,
        "keyframe-block-no-duplicate-selectors": true,
        "no-duplicate-at-import-rules": true,
        "no-duplicate-selectors": true,

        // No Empty
        "block-no-empty":true,
        "comment-no-empty": true,
        "no-empty-source": true,
        
        // No Invalid
        "color-no-invalid-hex": true,
        "function-calc-no-unspaced-operator": true,
        "keyframe-declaration-no-important": true,
        "media-query-no-invalid": true,
        "named-grid-areas-no-invalid": true,
        "no-invalid-double-slash-comments": true,
        "no-invalid-position-at-import-rule": true,
        "string-no-newline": true,
        
        
        // No Missing
        "custom-property-no-missing-var-function": true,
        "font-family-no-missing-generic-family-keyword": true,
        
        // No Nonstandard
        "function-linear-gradient-no-nonstandard-direction":true,

        // No Override
        "declaration-block-no-shorthand-property-overrides": true,
        
        // No Unmatchable
        "selector-anb-no-unmatchable": true,
        
        // Length
        "length-zero-no-unit":true,
        
        // 颜色函数与透明度
        "color-function-notation": "modern", // 使用现代颜色函数表示法 rgba(27 31 36 / 15%)
        "alpha-value-notation": "percentage", // 使用百分比表示透明度 15% 而非 0.15

        // 媒体查询
        "media-feature-range-notation": "context", // 使用上下文表示法 (width >= 480px)

        // 属性排序
        "order/properties-alphabetical-order": true,

        // 空行规则
        // 控制属性声明前是否应该有空行
        "declaration-empty-line-before": "always",
        // 控制选择器规则（整个样式块）前是否应该有空行
        "rule-empty-line-before": [
            "always",
            {
                except: ["first-nested"],
                ignore: ["after-comment"]
            }
        ],
        // 控制注释前是否应该有空行
        "comment-empty-line-before": [
            "always",
            {
                except: ["first-nested"],
                ignore: ["stylelint-commands", "after-comment"]
            }
        ],
        // 未知类型选择器错误
        "selector-type-no-unknown": [
            true,
            {
                // 允许自定义标签名
                ignoreTypes: ["hide", "spoiler"]
            }
        ],
        "max-nesting-depth": 99, // 限制嵌套深度
        "color-hex-length": "long",
        "declaration-block-single-line-max-declarations": 1, // 单行最多声明数
        "unit-allowed-list": ["px", "em", "rem", "%", "s", "vh", "vw", "deg", "fr", "vmax", "vmin", "ms"] // 允许的单位
    }
};
