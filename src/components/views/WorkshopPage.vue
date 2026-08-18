<template>
  <!-- 云端创意工坊：独立全屏页，主菜单「创意工坊」二级菜单进入（原设置页 workshop 标签迁移） -->
  <div class="workshop-page">
    <!-- 背景层（与主菜单同一张背景图 + 暗色遮罩保证对比度） -->
    <div class="workshop-page__bg"></div>

    <div class="relative
      z-10
      flex
      h-full
      w-full
      flex-col
      gap-4
      p-4
      md:p-8">
      <!-- 顶部：返回 + 标题 -->
      <div class="flex
        shrink-0
        items-center
        justify-between
        gap-4">
        <button
          class="inline-flex
            shrink-0
            items-center
            gap-2
            rounded-xl
            border
            border-white/15
            bg-white/8
            px-4
            py-2
            text-[0.9rem]
            text-white/80
            backdrop-blur-xl
            transition-all
            duration-200
            hover:bg-white/15
            hover:text-white"
          @click="goBack"
        >
          ← {{ t('views.menu.back') }}
        </button>
        <h1
          class="flex-1
            truncate
            text-center
            text-2xl
            font-bold
            text-white
            drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)]
            md:text-3xl"
        >
          {{ t('views.menu.cloudWorkshop') }}
        </h1>
        <div class="w-16
          shrink-0
          md:w-20"></div>
      </div>

      <!-- 内容容器（毛玻璃卡片流，原 SettingsWorkshop 内容） -->
      <div
        class="min-h-0
          flex-1
          overflow-y-auto
          custom-scrollbar
          rounded-2xl
          border
          border-white/10
          bg-white/8
          p-4
          backdrop-blur-2xl
          md:p-6"
      >
        <!-- Tab 切换：创意工坊讨论 / 市场商店 -->
        <div class="mb-4
          flex
          shrink-0
          items-center
          gap-1.5">
          <button
            class="cursor-pointer
              rounded-lg
              border
              border-white/10
              bg-white/6
              px-4
              py-1.5
              text-sm
              font-semibold
              text-white/60
              transition-all
              duration-200
              hover:bg-white/12
              hover:text-white/85
              [&.active]:border-[color:var(--cat-color,#79d9ff)]
              [&.active]:bg-[color:var(--cat-bg,rgba(121,217,255,0.15))]
              [&.active]:text-[color:var(--cat-color,#79d9ff)]"
            :class="{ active: activeTab === 'discussions' }"
            @click="activeTab = 'discussions'"
          >
            {{ $t('settings.workshop.discussionsTab') }}
          </button>
          <button
            class="cursor-pointer
              rounded-lg
              border
              border-white/10
              bg-white/6
              px-4
              py-1.5
              text-sm
              font-semibold
              text-white/60
              transition-all
              duration-200
              hover:bg-white/12
              hover:text-white/85
              [&.active]:border-[color:var(--cat-color,#79d9ff)]
              [&.active]:bg-[color:var(--cat-bg,rgba(121,217,255,0.15))]
              [&.active]:text-[color:var(--cat-color,#79d9ff)]"
            :class="{ active: activeTab === 'market' }"
            @click="activeTab = 'market'"
          >
            {{ $t('settings.workshop.marketTab') }}
          </button>
        </div>

        <div v-show="activeTab === 'discussions'">
        <!-- Toolbar: category filter + sort toggle -->
        <div class="mb-5
          flex
          shrink-0
          flex-wrap
          items-center
          justify-between
          gap-2">
          <div class="flex
            flex-wrap
            items-center
            gap-1.5">
            <button
              class="cursor-pointer
                rounded-md
                border
                border-transparent
                bg-white/6
                px-3
                py-1
                text-[13px]
                font-semibold
                tracking-[0.3px]
                text-white/50
                transition-all
                duration-200
                ease-in-out
                hover:bg-white/12
                hover:text-white/80
                [&.active]:border-[color:var(--cat-color,#79d9ff)]
                [&.active]:bg-[color:var(--cat-bg,rgba(121,217,255,0.15))]
                [&.active]:text-[color:var(--cat-color,#79d9ff)]
                [&.active]:hover:bg-[color:var(--cat-color,#79d9ff)]
                [&.active]:hover:text-white"
              :class="{ active: selectedCategory === null }"
              @click="selectCategory(null)"
            >
              {{ $t('settings.workshop.all') }}
            </button>
            <button
              v-for="cat in categories"
              :key="cat.name"
              class="cursor-pointer
                rounded-md
                border
                border-transparent
                bg-white/6
                px-3
                py-1
                text-[13px]
                font-semibold
                tracking-[0.3px]
                text-white/50
                transition-all
                duration-200
                ease-in-out
                hover:bg-white/12
                hover:text-white/80
                [&.active]:border-[color:var(--cat-color,#79d9ff)]
                [&.active]:bg-[color:var(--cat-bg,rgba(121,217,255,0.15))]
                [&.active]:text-[color:var(--cat-color,#79d9ff)]
                [&.active]:hover:bg-[color:var(--cat-color,#79d9ff)]
                [&.active]:hover:text-white"
              :class="{ active: selectedCategory === cat.name }"
              :style="{
                '--cat-color': cat.color,
                '--cat-bg': cat.color + '22',
              }"
              @click="selectCategory(cat.name)"
            >
              {{ cat.name }}
            </button>
            <span class="text-sm
              text-white/40
              ml-2"
              >{{ filteredDiscussions.length }} / {{ discussions.length }}</span
            >
          </div>

          <!-- Sort toggle -->
          <div class="flex
            items-center
            gap-1
            rounded-lg
            bg-white/5
            p-0.5">
            <button
              class="cursor-pointer
                rounded-md
                border-none
                bg-transparent
                px-2.5
                py-[3px]
                text-xs
                font-semibold
                text-white/40
                transition-all
                duration-200
                ease-in-out
                hover:text-white/70
                [&.active]:bg-white/10
                [&.active]:text-white"
              :class="{ active: sortMode === 'hot' }"
              @click="sortMode = 'hot'"
            >
              {{ $t('settings.workshop.hot') }}
            </button>
            <button
              class="cursor-pointer
                rounded-md
                border-none
                bg-transparent
                px-2.5
                py-[3px]
                text-xs
                font-semibold
                text-white/40
                transition-all
                duration-200
                ease-in-out
                hover:text-white/70
                [&.active]:bg-white/10
                [&.active]:text-white"
              :class="{ active: sortMode === 'newest' }"
              @click="sortMode = 'newest'"
            >
              {{ $t('settings.workshop.newest') }}
            </button>
          </div>
        </div>

        <!-- Loading -->
        <div
          v-if="loading"
          class="flex
            items-center
            justify-center
            py-12"
        >
          <p class="text-white/60">{{ $t('settings.workshop.loadingList') }}</p>
        </div>

        <!-- Error -->
        <div
          v-else-if="error"
          class="flex
            flex-col
            items-center
            justify-center
            gap-3
            py-12"
        >
          <p class="text-red-400">{{ error }}</p>
          <button
            class="rounded-lg
              border
              border-white/10
              bg-white/10
              px-5
              py-2
              text-white
              transition-colors
              hover:bg-white/20"
            @click="load"
          >
            {{ $t('settings.workshop.retry') }}
          </button>
        </div>

        <!-- Empty -->
        <div
          v-else-if="discussions.length === 0"
          class="flex
            items-center
            justify-center
            py-12"
        >
          <p class="text-white/50">{{ $t('settings.workshop.empty') }}</p>
        </div>

        <!-- Filtered empty -->
        <div
          v-else-if="filteredDiscussions.length === 0"
          class="flex
            items-center
            justify-center
            py-12"
        >
          <p class="text-white/50">{{ $t('settings.workshop.emptyCategory') }}</p>
        </div>

        <!-- Discussion cards section -->
        <template v-else>
          <!-- Token hint: no real upvote data -->
          <div
            v-if="!hasAnyUpvoteData"
            class="mb-5
              flex
              items-center
              gap-3
              rounded-xl
              border
              border-yellow-500/25
              bg-yellow-500/10
              px-5
              py-3
              text-sm
              text-yellow-200/80"
          >
            <span class="text-base">💡</span>
            <span>
              {{ $t('settings.workshop.upvoteHint1')
              }}<strong>{{ $t('settings.workshop.upvoteHintLink') }}</strong
              >{{ $t('settings.workshop.upvoteHint2') }}
            </span>
          </div>

          <div class="grid
            w-full
            gap-5
            grid-cols-1
            xl:grid-cols-2">
            <div
              v-for="discussion in pagedDiscussions"
              :key="discussion.number"
              class="group
                relative
                flex
                items-start
                rounded-2xl
                border
                border-white/10
                bg-white/10
                p-5
                backdrop-blur-xl
                transition-all
                duration-300
                hover:-translate-y-0.5
                hover:border-white/20
                hover:shadow-xl
                hover:shadow-white/5
                cursor-pointer"
              @click="openDiscussion(discussion.html_url)"
            >
              <!-- Top-left: category icon -->
              <div
                v-if="getCornerIcon(discussion.category.name)"
                class="absolute
                  -top-2
                  -left-2
                  z-10
                  flex
                  w-6
                  h-6
                  -rotate-18
                  items-center
                  justify-center
                  rounded-full
                  text-brand
                  shadow-md"
              >
                <component
                  :is="getCornerIcon(discussion.category.name)"
                  :size="20"
                />
              </div>

              <!-- Top-right: external link -->
              <button
                class="absolute
                  top-3
                  right-3
                  z-10
                  rounded-full
                  bg-white/5
                  p-1.5
                  text-white/40
                  transition-all
                  hover:bg-white/10
                  hover:text-white"
                @click.stop="openDiscussion(discussion.html_url)"
              >
                <ExternalLink :size="14" />
              </button>

              <!-- Left: Avatar section -->
              <div
                class="flex
                  w-32
                  shrink-0
                  flex-col
                  items-center
                  gap-3
                  border-r
                  border-white/10
                  pr-5"
              >
                <div
                  class="h-28
                    w-28
                    shrink-0
                    overflow-hidden
                    rounded-full
                    border-2
                    border-white/20
                    shadow-lg"
                >
                  <img
                    v-if="discussion.avatar_url"
                    :src="discussion.avatar_url"
                    :alt="discussion.title"
                    class="h-full
                      w-full
                      object-cover
                      transition-transform
                      duration-500
                      group-hover:scale-110"
                  />
                  <div
                    v-else
                    class="flex
                      h-full
                      w-full
                      items-center
                      justify-center
                      bg-white/5"
                  >
                    <img
                      src="@/assets/images/LingChatLogo.png"
                      alt="Logo"
                      class="h-full
                        w-full
                        -rotate-20
                        scale-130
                        object-contain
                        opacity-100"
                    />
                  </div>
                </div>
                <!-- Category badge -->
                <span
                  class="rounded-full
                    border
                    px-3
                    py-0.5
                    text-center
                    text-sm
                    font-medium
                    leading-5"
                  :style="{
                    backgroundColor: getCategoryColor(discussion.category.name) + '22',
                    borderColor: getCategoryColor(discussion.category.name) + '4D',
                    color: getCategoryColor(discussion.category.name),
                  }"
                >
                  {{ discussion.category.name }}
                </span>
              </div>

              <!-- Right: Content -->
              <div class="flex
                h-full
                min-w-0
                flex-1
                flex-col
                py-0.5
                pl-4">
                <!-- Title -->
                <h3 class="mb-2
                  line-clamp-2
                  text-xl
                  font-bold
                  leading-7
                  text-white">
                  {{ discussion.title }}
                </h3>

                <!-- Description -->
                <p class="mb-3
                  line-clamp-4
                  flex-1
                  text-base
                  leading-5
                  text-white/60">
                  {{ getDisplayDescription(discussion) }}
                </p>

                <!-- Footer: tags -->
                <div
                  v-if="discussion.tags.length > 0"
                  class="mb-2
                    flex
                    min-h-5
                    flex-wrap
                    items-center
                    gap-1.5"
                >
                  <span
                    v-for="(tag, i) in discussion.tags"
                    :key="tag"
                    class="rounded-full
                      border
                      px-2
                      py-0.5
                      text-xs
                      font-medium"
                    :style="{
                      backgroundColor: getTagColor(i) + '22',
                      borderColor: getTagColor(i) + '4D',
                      color: getTagColor(i),
                    }"
                  >
                    {{ tag }}
                  </span>
                </div>

                <!-- Footer: meta info -->
                <div
                  class="flex
                    items-center
                    gap-4
                    border-t
                    border-white/5
                    pt-2.5
                    text-xs
                    text-white/35"
                >
                  <!-- Upvotes -->
                  <span
                    class="flex
                      items-center
                      gap-1"
                    :title="
                      discussion.has_upvotes
                        ? $t('settings.workshop.upvoteTitle')
                        : $t('settings.workshop.reactionTitle')
                    "
                  >
                    <ThumbsUp :size="12" />
                    {{ discussion.has_upvotes ? discussion.upvotes : discussion.reactions_upvotes }}
                  </span>
                  <!-- Author -->
                  <span class="flex
                    items-center
                    gap-1">
                    <User :size="12" />
                    {{ discussion.author?.login ?? $t('settings.workshop.unknownAuthor') }}
                  </span>
                  <!-- Time -->
                  <span class="ml-auto
                    flex
                    items-center
                    gap-1">
                    <Clock :size="12" />
                    {{ formatTime(discussion.created_at) }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </template>

        <!-- Pagination -->
        <div
          v-if="totalPages > 1"
          class="mt-2
            flex
            w-full
            items-center
            justify-between
            px-3
            py-2"
        >
          <button
            class="rounded-lg
              border-none
              bg-white/8
              px-5
              py-2
              text-base
              font-medium
              text-white/60
              transition-all
              duration-200
              hover:bg-white/15
              hover:text-white
              disabled:cursor-not-allowed
              disabled:opacity-30"
            :disabled="currentPage <= 1"
            @click="currentPage--"
          >
            {{ $t('settings.shared.prevPage') }}
          </button>
          <span class="text-base
            font-medium
            text-white/60">
            {{ $t('settings.shared.pageOf', { current: currentPage, total: totalPages }) }}
          </span>
          <button
            class="rounded-lg
              border-none
              bg-white/8
              px-5
              py-2
              text-base
              font-medium
              text-white/60
              transition-all
              duration-200
              hover:bg-white/15
              hover:text-white
              disabled:cursor-not-allowed
              disabled:opacity-30"
            :disabled="currentPage >= totalPages"
            @click="currentPage++"
          >
            {{ $t('settings.shared.nextPage') }}
          </button>
        </div>

        <!-- Refresh button -->
        <div
          v-if="!loading && !error"
          class="mt-6
            flex
            justify-center"
        >
          <button
            class="rounded-lg
              border
              border-white/5
              bg-white/5
              px-5
              py-1.5
              text-sm
              text-white/40
              transition-all
              hover:border-white/15
              hover:bg-white/10
              hover:text-white/70"
            @click="load"
          >
            {{ $t('settings.workshop.refreshList') }}
          </button>
        </div>
        </div>
        <!-- /讨论区块 -->

        <!-- 市场商店区块 -->
        <div v-show="activeTab === 'market'" class="flex
          h-full
          flex-col">
          <!-- 工具栏：类型筛选 + 刷新 -->
          <div class="mb-4
            flex
            shrink-0
            flex-wrap
            items-center
            justify-between
            gap-2">
            <div class="flex
              flex-wrap
              items-center
              gap-1.5">
              <button
                v-for="f in marketFilters"
                :key="f.type ?? 'all'"
                class="cursor-pointer
                  rounded-md
                  border
                  border-transparent
                  bg-white/6
                  px-3
                  py-1
                  text-[13px]
                  font-semibold
                  tracking-[0.3px]
                  text-white/50
                  transition-all
                  duration-200
                  ease-in-out
                  hover:bg-white/12
                  hover:text-white/80
                  [&.active]:border-[color:var(--cat-color,#79d9ff)]
                  [&.active]:bg-[color:var(--cat-bg,rgba(121,217,255,0.15))]
                  [&.active]:text-[color:var(--cat-color,#79d9ff)]
                  [&.active]:hover:bg-[color:var(--cat-color,#79d9ff)]
                  [&.active]:hover:text-white"
                :class="{ active: marketFilter === f.type }"
                :style="f.type ? { '--cat-color': typeColor(f.type).fg, '--cat-bg': typeColor(f.type).bg } : {}"
                @click="marketFilter = f.type"
              >
                {{ f.label }}
                <span class="ml-1
                  text-[11px]
                  font-normal
                  opacity-60">{{ f.count }}</span>
              </button>
            </div>
            <button
              class="cursor-pointer
                rounded-md
                border
                border-white/10
                bg-white/6
                px-3
                py-1
                text-xs
                font-semibold
                text-white/50
                transition-all
                duration-200
                hover:bg-white/12
                hover:text-white/80"
              @click="loadMarket"
            >
              {{ $t('settings.workshop.refreshList') }}
            </button>
          </div>

          <!-- 搜索框 + 市场说明 -->
          <div class="mb-4
            flex
            shrink-0
            items-center
            gap-3">
            <div class="relative
              flex-1">
              <Search
                :size="15"
                class="pointer-events-none
                  absolute
                  left-3
                  top-1/2
                  -translate-y-1/2
                  text-white/35"
              />
              <input
                v-model="marketQuery"
                type="text"
                :placeholder="$t('settings.workshop.searchPlaceholder')"
                class="w-full
                  rounded-lg
                  border
                  border-white/10
                  bg-white/6
                  py-2
                  pl-9
                  pr-3
                  text-sm
                  text-white
                  placeholder:text-white/30
                  outline-none
                  transition-all
                  duration-200
                  focus:border-white/25
                  focus:bg-white/10"
              />
            </div>
            <span class="hidden
              shrink-0
              text-xs
              text-white/35
              md:block">{{ $t('settings.workshop.marketHint') }}</span>
          </div>

          <!-- Loading -->
          <div
            v-if="marketLoading"
            class="flex
              items-center
              justify-center
              py-12"
          >
            <p class="text-white/60">{{ $t('settings.workshop.loadingList') }}</p>
          </div>

          <!-- Error -->
          <div
            v-else-if="marketError"
            class="flex
              flex-col
              items-center
              justify-center
              gap-3
              py-12"
          >
            <p class="text-red-400">{{ marketError }}</p>
            <button
              class="rounded-lg
                border
                border-white/10
                bg-white/10
                px-5
                py-2
                text-white
                transition-colors
                hover:bg-white/20"
              @click="loadMarket"
            >
              {{ $t('settings.workshop.retry') }}
            </button>
          </div>

          <!-- Empty -->
          <div
            v-else-if="marketPackages.length === 0"
            class="flex
              items-center
              justify-center
              py-12"
          >
            <p class="text-white/50">{{ $t('settings.workshop.marketEmpty') }}</p>
          </div>

          <!-- 筛选/搜索无结果 -->
          <div
            v-else-if="filteredMarketPackages.length === 0"
            class="flex
              items-center
              justify-center
              py-12"
          >
            <p class="text-white/50">{{ $t('settings.workshop.marketNoMatch') }}</p>
          </div>

          <!-- 包卡片网格（风格对齐创意工坊讨论卡片：左侧封面区 + 右侧信息区） -->
          <div
            v-else
            class="grid
              w-full
              gap-5
              grid-cols-1
              xl:grid-cols-2"
          >
            <div
              v-for="pkg in filteredMarketPackages"
              :key="pkg.id"
              class="group
                relative
                flex
                items-start
                rounded-2xl
                border
                border-white/10
                bg-white/10
                p-5
                backdrop-blur-xl
                transition-all
                duration-300
                hover:-translate-y-0.5
                hover:border-white/20
                hover:shadow-xl
                hover:shadow-white/5
                cursor-pointer"
              @click="openDetail(pkg)"
            >
              <!-- 左侧：封面区（类型主色渐变 + 图标 + 类型徽章） -->
              <div class="flex
                w-28
                shrink-0
                flex-col
                items-center
                gap-3
                border-r
                border-white/10
                pr-5">
                <div
                  class="flex
                    h-24
                    w-24
                    shrink-0
                    items-center
                    justify-center
                    overflow-hidden
                    rounded-2xl
                    border
                    border-white/15
                    shadow-lg
                    transition-transform
                    duration-500
                    group-hover:scale-105"
                  :style="{ background: typeCover(pkg.type) }"
                >
                  <component
                    :is="typeIcon(pkg.type)"
                    :size="40"
                    class="text-white/90
                      drop-shadow-md"
                  />
                </div>
                <span
                  class="rounded-full
                    border
                    px-3
                    py-0.5
                    text-center
                    text-sm
                    font-medium
                    leading-5"
                  :style="{
                    backgroundColor: typeColor(pkg.type).bg,
                    borderColor: typeColor(pkg.type).fg + '4D',
                    color: typeColor(pkg.type).fg,
                  }"
                >
                  {{ typeLabel(pkg.type) }}
                </span>
              </div>

              <!-- 右侧：信息区 -->
              <div class="flex
                h-full
                min-w-0
                flex-1
                flex-col
                py-0.5
                pl-4">
                <!-- 名称 + 版本 -->
                <div class="flex
                  items-center
                  justify-between
                  gap-2">
                  <h3 class="line-clamp-1
                    text-base
                    font-bold
                    text-white">{{ pkg.name }}</h3>
                  <span class="shrink-0
                    text-xs
                    text-white/40">v{{ pkg.version }}</span>
                </div>
                <!-- 描述 -->
                <p class="mt-1.5
                  line-clamp-2
                  text-sm
                  leading-relaxed
                  text-white/60">{{ pkg.description || $t('settings.workshop.noDesc') }}</p>
                <!-- 作者 -->
                <p class="mt-1
                  text-xs
                  text-white/35">{{ pkg.author }}</p>

                <!-- 底部：状态 + 操作 -->
                <div class="mt-auto
                  flex
                  items-center
                  justify-between
                  gap-2
                  border-t
                  border-white/5
                  pt-2.5">
                  <template v-if="installingId === pkg.id">
                    <template v-if="progressPhase[pkg.id] === 'install'">
                      <span class="flex
                        items-center
                        gap-1.5
                        text-xs
                        text-white/60">
                        {{ $t('settings.workshop.installing') }}
                        <span class="flex
                          items-center
                          gap-1">
                          <span class="h-1.5
                            w-1.5
                            animate-pulse
                            rounded-full
                            bg-[color:var(--cat-color,#79d9ff)]"></span>
                          <span class="h-1.5
                            w-1.5
                            animate-pulse
                            rounded-full
                            bg-[color:var(--cat-color,#79d9ff)] [animation-delay:150ms]"></span>
                          <span class="h-1.5
                            w-1.5
                            animate-pulse
                            rounded-full
                            bg-[color:var(--cat-color,#79d9ff)] [animation-delay:300ms]"></span>
                        </span>
                      </span>
                    </template>
                    <template v-else>
                      <span class="flex
                        items-center
                        gap-2">
                        <span class="text-xs
                          text-white/50">
                          {{ progressPercent[pkg.id] ?? 0 }}%
                          <template v-if="progressBytes[pkg.id]">
                            · {{ formatBytes(progressBytes[pkg.id]) }}
                          </template>
                        </span>
                        <span class="h-1
                          w-24
                          overflow-hidden
                          rounded-full
                          bg-white/10">
                          <span
                            class="block
                              h-full
                              rounded-full
                              bg-[color:var(--cat-color,#79d9ff)]
                              transition-all
                              duration-200"
                            :style="{ width: (progressPercent[pkg.id] ?? 0) + '%' }"
                          ></span>
                        </span>
                      </span>
                    </template>
                  </template>
                  <template v-else-if="installedMap[pkg.id]">
                    <span class="flex
                      items-center
                      gap-1.5
                      text-xs
                      text-emerald-300/80">
                      <CheckCircle2 :size="13" />
                      {{ $t('settings.workshop.installed') }}
                      <template v-if="installedMap[pkg.id].version !== pkg.version">(v{{ pkg.version }})</template>
                    </span>
                    <button
                      class="rounded-lg
                        border
                        border-white/10
                        bg-white/5
                        px-3
                        py-1
                        text-xs
                        text-white/50
                        transition-colors
                        hover:bg-white/10
                        hover:text-red-300"
                      :disabled="installingId !== null"
                      @click.stop="uninstallPkg(pkg.id)"
                    >
                      {{ $t('settings.workshop.uninstall') }}
                    </button>
                  </template>
                  <template v-else>
                    <span class="flex
                      items-center
                      gap-1.5
                      text-xs
                      text-white/35">
                      <Download :size="13" />
                      {{ formatBytes(pkg.size || 0) }}
                    </span>
                    <button
                      class="rounded-lg
                        border
                        border-white/10
                        bg-white/8
                        px-3.5
                        py-1
                        text-xs
                        font-semibold
                        text-white/70
                        transition-all
                        hover:border-[color:var(--cat-color,#79d9ff)]
                        hover:text-[color:var(--cat-color,#79d9ff)]
                        disabled:cursor-not-allowed
                        disabled:opacity-40"
                      :disabled="installingId !== null"
                      @click.stop="installPkg(pkg.id)"
                    >
                      {{ $t('settings.workshop.install') }}
                    </button>
                  </template>
                </div>
              </div>
            </div>
          </div>
        </div>
        <!-- /市场区块 -->
      </div>
    </div>
  </div>

  <!-- 包详情弹窗（点击商店卡片打开；manifest 字段动态渲染，为多类型铺路） -->
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="selectedPkg"
        class="fixed
          inset-0
          z-[10001]
          flex
          items-center
          justify-center
          p-4"
      >
        <!-- 遮罩 -->
        <div
          class="absolute
            inset-0
            bg-black/60
            backdrop-blur-sm"
          @click="selectedPkg = null"
        ></div>

        <!-- 弹窗主体 -->
        <div
          class="relative
            z-10
            max-h-[85vh]
            w-full
            max-w-lg
            overflow-y-auto
            custom-scrollbar
            rounded-2xl
            border
            border-white/15
            bg-[#141821]/95
            p-6
            shadow-2xl
            backdrop-blur-2xl"
        >
          <!-- 关闭 -->
          <button
            class="absolute
              right-4
              top-4
              z-10
              rounded-full
              bg-white/5
              p-1.5
              text-white/40
              transition-all
              hover:bg-white/10
              hover:text-white"
            @click="selectedPkg = null"
          >
            <X :size="16" />
          </button>

          <!-- 头部：封面 + 名称 + 类型徽章 + 版本/作者 -->
          <div class="flex
            items-center
            gap-4
            pr-8">
            <div
              class="flex
                h-16
                w-16
                shrink-0
                items-center
                justify-center
                overflow-hidden
                rounded-2xl
                border
                border-white/15
                shadow-lg"
              :style="{ background: typeCover(selectedPkg.type) }"
            >
              <component
                :is="typeIcon(selectedPkg.type)"
                :size="28"
                class="text-white/90
                  drop-shadow-md"
              />
            </div>
            <div class="min-w-0
              flex-1">
              <div class="flex
                flex-wrap
                items-center
                gap-2">
                <h3 class="truncate
                  text-xl
                  font-bold
                  text-white">{{ selectedPkg.name }}</h3>
                <span
                  class="rounded-full
                    border
                    px-2
                    py-0.5
                    text-[11px]
                    font-semibold"
                  :style="{
                    backgroundColor: typeColor(selectedPkg.type).bg,
                    borderColor: typeColor(selectedPkg.type).fg + '4D',
                    color: typeColor(selectedPkg.type).fg,
                  }"
                >{{ typeLabel(selectedPkg.type) }}</span>
              </div>
              <p class="mt-1
                text-xs
                text-white/40">v{{ selectedPkg.version }}<template v-if="selectedPkg.author"> · {{ selectedPkg.author }}</template></p>
            </div>
          </div>

          <!-- 完整描述 -->
          <p class="mt-4
            whitespace-pre-wrap
            text-sm
            leading-relaxed
            text-white/70">{{ selectedPkg.description || $t('settings.workshop.noDesc') }}</p>

          <!-- manifest 动态区：分类 / 标签 / 工具 -->
          <template v-if="manifestContent(selectedPkg).category || manifestTags(selectedPkg).length || manifestTools(selectedPkg).length">
            <div class="mt-5
              space-y-3
              border-t
              border-white/10
              pt-4">
              <!-- 分类 -->
              <div
                v-if="manifestContent(selectedPkg).category"
                class="flex
                  items-center
                  gap-2
                  text-sm"
              >
                <span class="shrink-0
                  text-white/35">{{ $t('settings.workshop.detailCategory') }}</span>
                <span
                  class="rounded-full
                    border
                    px-2.5
                    py-0.5
                    text-xs
                    font-medium"
                  :style="{
                    backgroundColor: getCategoryColor(manifestContent(selectedPkg).category!) + '22',
                    borderColor: getCategoryColor(manifestContent(selectedPkg).category!) + '4D',
                    color: getCategoryColor(manifestContent(selectedPkg).category!),
                  }"
                >{{ manifestContent(selectedPkg).category }}</span>
              </div>
              <!-- 标签（彩虹色，与讨论卡片同款） -->
              <div
                v-if="manifestTags(selectedPkg).length"
                class="flex
                  flex-wrap
                  items-center
                  gap-1.5"
              >
                <span class="shrink-0
                  text-sm
                  text-white/35">{{ $t('settings.workshop.detailTags') }}</span>
                <span
                  v-for="(tag, i) in manifestTags(selectedPkg)"
                  :key="tag"
                  class="rounded-full
                    border
                    px-2
                    py-0.5
                    text-xs
                    font-medium"
                  :style="{
                    backgroundColor: getTagColor(i) + '22',
                    borderColor: getTagColor(i) + '4D',
                    color: getTagColor(i),
                  }"
                >{{ tag }}</span>
              </div>
              <!-- 插件工具声明 -->
              <div v-if="manifestTools(selectedPkg).length">
                <p class="mb-2
                  text-sm
                  text-white/35">{{ $t('settings.workshop.detailTools') }}</p>
                <div class="space-y-2">
                  <div
                    v-for="tool in manifestTools(selectedPkg)"
                    :key="tool.name"
                    class="rounded-xl
                      border
                      border-white/10
                      bg-white/5
                      px-3
                      py-2"
                  >
                    <p class="text-sm
                      font-semibold
                      text-white/85">{{ tool.name }}</p>
                    <p class="mt-0.5
                      text-xs
                      leading-relaxed
                      text-white/50">{{ tool.description }}</p>
                  </div>
                </div>
              </div>
            </div>
          </template>

          <!-- 元信息 -->
          <div class="mt-5
            space-y-1.5
            rounded-xl
            border
            border-white/10
            bg-white/5
            p-3
            text-xs
            text-white/45">
            <div class="flex
              items-center
              justify-between
              gap-3">
              <span>{{ $t('settings.workshop.detailSize') }}</span>
              <span class="text-white/70">{{ formatBytes(selectedPkg.size || 0) }}</span>
            </div>
            <div
              v-if="selectedPkg.sha256"
              class="flex
                items-center
                justify-between
                gap-3"
            >
              <span>{{ $t('settings.workshop.detailSha256') }}</span>
              <span
                class="max-w-[60%]
                  truncate
                  font-mono
                  text-white/60"
                :title="selectedPkg.sha256"
              >{{ selectedPkg.sha256 }}</span>
            </div>
            <div
              v-if="selectedPkg.review_report_url"
              class="flex
                items-center
                justify-between
                gap-3"
            >
              <span>{{ $t('settings.workshop.detailReview') }}</span>
              <button
                class="flex
                  items-center
                  gap-1
                  text-[color:var(--cat-color,#79d9ff)]
                  hover:underline"
                @click="openUrl(selectedPkg.review_report_url!)"
              >
                <ExternalLink :size="12" />
                {{ $t('settings.workshop.detailOpen') }}
              </button>
            </div>
          </div>

          <!-- 底部操作 -->
          <div class="mt-5
            flex
            items-center
            justify-end
            gap-2.5">
            <button
              class="rounded-xl
                border
                border-white/15
                bg-white/8
                px-5
                py-2
                text-sm
                font-semibold
                text-white/70
                transition-all
                hover:bg-white/15
                hover:text-white"
              @click="selectedPkg = null"
            >
              {{ $t('settings.workshop.detailClose') }}
            </button>
            <template v-if="installingId === selectedPkg.id">
              <span class="flex
                items-center
                gap-1.5
                text-xs
                text-white/60">
                {{ $t('settings.workshop.installing') }}
                <span class="flex
                  items-center
                  gap-1">
                  <span class="h-1.5
                    w-1.5
                    animate-pulse
                    rounded-full
                    bg-[color:var(--cat-color,#79d9ff)]"></span>
                  <span class="h-1.5
                    w-1.5
                    animate-pulse
                    rounded-full
                    bg-[color:var(--cat-color,#79d9ff)] [animation-delay:150ms]"></span>
                  <span class="h-1.5
                    w-1.5
                    animate-pulse
                    rounded-full
                    bg-[color:var(--cat-color,#79d9ff)] [animation-delay:300ms]"></span>
                </span>
              </span>
            </template>
            <button
              v-else-if="installedMap[selectedPkg.id]"
              class="rounded-xl
                border
                border-white/10
                bg-white/5
                px-4
                py-2
                text-sm
                text-white/60
                transition-colors
                hover:bg-white/10
                hover:text-red-300"
              :disabled="installingId !== null"
              @click="uninstallPkg(selectedPkg.id)"
            >
              {{ $t('settings.workshop.uninstall') }}
            </button>
            <button
              v-else
              class="rounded-xl
                border
                border-white/10
                bg-white/8
                px-4
                py-2
                text-sm
                font-semibold
                text-white/70
                transition-all
                hover:border-[color:var(--cat-color,#79d9ff)]
                hover:text-[color:var(--cat-color,#79d9ff)]
                disabled:cursor-not-allowed
                disabled:opacity-40"
              :disabled="installingId !== null"
              @click="installPkg(selectedPkg.id)"
            >
              {{ $t('settings.workshop.install') }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import { useUIStore } from '@/stores/modules/ui/ui'
import { fetchDiscussions, type Discussion } from '@/api/services/workshop'
import {
  fetchMarketIndex,
  fetchInstalled,
  installPackage,
  uninstallPackage,
  onMarketProgress,
  notifyMarketChanged,
  type MarketPackage,
  type InstalledRecord,
} from '@/api/services/market'
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  Cat,
  CheckCircle2,
  Clock,
  Clover,
  Download,
  ExternalLink,
  Mic,
  Package,
  Puzzle,
  ScrollText,
  Search,
  ThumbsUp,
  User,
  X,
} from 'lucide-vue-next'
import type { Component } from 'vue'

// ── Data ──────────────────────────────────────────────────────

const discussions = ref<Discussion[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const selectedCategory = ref<string | null>(null)
const currentPage = ref(1)
const sortMode = ref<'hot' | 'newest'>('hot')
const { t } = useI18n()
const router = useRouter()
const dialogStore = useDialogStore()
const uiStore = useUIStore()
const ITEMS_PER_PAGE = 10

// ── Market ────────────────────────────────────────────────────

const activeTab = ref<'discussions' | 'market'>('discussions')
const marketPackages = ref<MarketPackage[]>([])
const installedMap = ref<Record<string, InstalledRecord>>({})
const marketLoading = ref(false)
const marketError = ref<string | null>(null)
const installingId = ref<string | null>(null)
const progressPercent = ref<Record<string, number>>({})
const progressBytes = ref<Record<string, number>>({})
const progressPhase = ref<Record<string, string>>({})
let unlistenProgress: (() => void) | null = null

// 包详情弹窗
const selectedPkg = ref<MarketPackage | null>(null)
const openDetail = (pkg: MarketPackage) => {
  selectedPkg.value = pkg
}

function typeLabel(type: string): string {
  const map: Record<string, string> = {
    plugin: '插件',
    character: '角色',
    script: '剧本',
    voice: '语音',
  }
  return map[type] || type
}

/** 字节数 → 人类可读（B/KB/MB/GB） */
function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)))
  const v = bytes / 1024 ** i
  return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`
}

function typeColor(type: string): { bg: string; fg: string } {
  const map: Record<string, { bg: string; fg: string }> = {
    plugin: { bg: 'rgba(74,222,128,0.15)', fg: '#4ade80' },
    character: { bg: 'rgba(121,217,255,0.15)', fg: '#79d9ff' },
    script: { bg: 'rgba(168,85,247,0.15)', fg: '#a855f7' },
    voice: { bg: 'rgba(234,179,8,0.15)', fg: '#eab308' },
  }
  return map[type] || { bg: 'rgba(107,114,128,0.15)', fg: '#6b7280' }
}

/** 类型 → 封面渐变（市场数据暂无封面图素材，用类型主色渐变 + 图标生成封面） */
function typeCover(type: string): string {
  const fg = typeColor(type).fg
  return `linear-gradient(135deg, ${fg}59 0%, ${fg}14 100%)`
}

/** 类型 → 封面图标 */
function typeIcon(type: string): Component {
  switch (type) {
    case 'plugin':
      return Puzzle
    case 'character':
      return Cat
    case 'script':
      return ScrollText
    case 'voice':
      return Mic
    default:
      return Package
  }
}

// ── manifest 动态读取（为更多类型铺路：新类型在 manifest 里加字段即自动展示） ──

interface ManifestContent {
  category?: string
  tags: string[]
}

/** manifest 的 content 段（分类/标签）——未知结构安全降级为空 */
function manifestContent(pkg: MarketPackage): ManifestContent {
  const m = pkg.manifest
  if (!m || typeof m !== 'object') return { tags: [] }
  const content = (m as Record<string, unknown>).content
  if (!content || typeof content !== 'object') return { tags: [] }
  const c = content as Record<string, unknown>
  return {
    category: typeof c.category === 'string' ? c.category : undefined,
    tags: Array.isArray(c.tags)
      ? c.tags.filter((x): x is string => typeof x === 'string')
      : [],
  }
}

function manifestTags(pkg: MarketPackage): string[] {
  return manifestContent(pkg).tags
}

/** manifest 的 tools 段（插件声明，只读无副作用）——未知结构安全降级为空 */
function manifestTools(
  pkg: MarketPackage,
): { name: string; description: string }[] {
  const m = pkg.manifest
  if (!m || typeof m !== 'object') return []
  const tools = (m as Record<string, unknown>).tools
  if (!Array.isArray(tools)) return []
  return tools.flatMap((t) => {
    if (!t || typeof t !== 'object') return []
    const rec = t as Record<string, unknown>
    const name = typeof rec.name === 'string' ? rec.name : ''
    if (!name) return []
    return [{ name, description: typeof rec.description === 'string' ? rec.description : '' }]
  })
}

/** 类型筛选：全部 + 实际存在的各类型（含数量），顺序按约定类型排前 */
const marketFilter = ref<string | null>(null)
const marketQuery = ref('')

const marketFilters = computed(() => {
  const counts: Record<string, number> = {}
  for (const p of marketPackages.value) {
    counts[p.type] = (counts[p.type] || 0) + 1
  }
  const order = ['character', 'script', 'plugin', 'voice']
  const seen = new Set<string>()
  const types: string[] = []
  for (const type of [...order, ...Object.keys(counts)]) {
    if (counts[type] && !seen.has(type)) {
      seen.add(type)
      types.push(type)
    }
  }
  return [
    { type: null as string | null, label: t('settings.workshop.all'), count: marketPackages.value.length },
    ...types.map((type) => ({ type, label: typeLabel(type), count: counts[type] })),
  ]
})

/** 类型筛选 + 关键词（名称/作者/描述）过滤后的包列表 */
const filteredMarketPackages = computed(() => {
  const q = marketQuery.value.trim().toLowerCase()
  return marketPackages.value.filter((pkg) => {
    if (marketFilter.value && pkg.type !== marketFilter.value) return false
    if (!q) return true
    return `${pkg.name} ${pkg.author ?? ''} ${pkg.description ?? ''}`
      .toLowerCase()
      .includes(q)
  })
})

async function loadInstalled() {
  try {
    const records = await fetchInstalled()
    const map: Record<string, InstalledRecord> = {}
    for (const r of records) map[r.id] = r
    installedMap.value = map
  } catch {
    // 已装列表失败不阻塞商店展示
  }
}

async function loadMarket() {
  marketLoading.value = true
  marketError.value = null
  try {
    const [pkgs] = await Promise.all([fetchMarketIndex(), loadInstalled()])
    marketPackages.value = pkgs
  } catch (e: unknown) {
    const err = e as { message?: string }
    marketError.value = typeof e === 'string' ? e : err?.message || t('settings.workshop.loadFailed')
  } finally {
    marketLoading.value = false
  }
}

async function installPkg(id: string) {
  if (installingId.value) return
  installingId.value = id
  progressPercent.value[id] = 0
  progressBytes.value[id] = 0
  progressPhase.value[id] = 'download'
  try {
    await installPackage(id)
    progressPercent.value[id] = 100
    await loadInstalled()
    notifyMarketChanged()
    uiStore.showSuccess({ title: t('settings.workshop.installSuccess') })
  } catch (e: unknown) {
    const err = e as { message?: string }
    uiStore.showError({
      title: t('settings.workshop.installFailed'),
      message: typeof e === 'string' ? e : err?.message,
    })
  } finally {
    installingId.value = null
  }
}

async function uninstallPkg(id: string) {
  if (installingId.value) return
  const pkg = marketPackages.value.find((p) => p.id === id)
  const confirmed = await dialogStore.confirm(
    t('settings.workshop.uninstallConfirm', { name: pkg?.name ?? id }),
  )
  if (!confirmed) return
  installingId.value = id
  try {
    await uninstallPackage(id)
    await loadInstalled()
    notifyMarketChanged()
    uiStore.showSuccess({ title: t('settings.workshop.uninstallSuccess') })
  } catch (e: unknown) {
    const err = e as { message?: string }
    uiStore.showError({
      title: t('settings.workshop.uninstallFailed'),
      message: typeof e === 'string' ? e : err?.message,
    })
  } finally {
    installingId.value = null
  }
}

const goBack = () => {
  // 从主菜单（或设置）进入后返回上一页；直接访问时兜底回主菜单
  if (window.history.length > 1) router.back()
  else router.push('/')
}

// ── Category colors ───────────────────────────────────────────

function getCategoryColor(name: string): string {
  const n = name.toLowerCase()
  if (/人物|角色|character/i.test(n)) return '#79d9ff'
  if (/剧本|故事|script|story/i.test(n)) return '#a855f7'
  if (/资源|工具|素材|模组|asset|tool|plugin|mod/i.test(n)) return '#4ade80'
  if (/背景|background/i.test(n)) return '#3b82f6'
  if (/音乐|music|bgm/i.test(n)) return '#ec4899'
  if (/语音|voice|tts/i.test(n)) return '#eab308'
  return '#6b7280'
}

const TAG_RAINBOW = [
  '#fca5a5', // 红
  '#fdba74', // 橙
  '#fde047', // 黄
  '#86efac', // 绿
  '#93c5fd', // 蓝
  '#a5b4fc', // 靛
  '#d8b4fe', // 紫
]

function getTagColor(index: number): string {
  return TAG_RAINBOW[index % TAG_RAINBOW.length]
}

function getCornerIcon(name: string): Component | null {
  const n = name.toLowerCase()
  if (/人物|角色|character/i.test(n)) return Cat
  if (/资源|工具|素材|模组|asset|tool|plugin|mod/i.test(n)) return Clover
  return null
}

// ── Categories ────────────────────────────────────────────────

const categories = computed(() => {
  const seen = new Set<string>()
  const result: { name: string; color: string }[] = []
  for (const d of discussions.value) {
    const name = d.category.name
    if (!seen.has(name)) {
      seen.add(name)
      result.push({ name, color: getCategoryColor(name) })
    }
  }
  return result
})

// ── Sort → Filter → Pagination ────────────────────────────────

const hasAnyUpvoteData = computed(() => discussions.value.some((d) => d.has_upvotes))

const sortedDiscussions = computed(() => {
  const arr = [...discussions.value]
  if (sortMode.value === 'hot') {
    // 优先用真实 upvotes，没有则用 👍 表情数
    arr.sort((a, b) => {
      const aScore = a.has_upvotes ? a.upvotes : a.reactions_upvotes
      const bScore = b.has_upvotes ? b.upvotes : b.reactions_upvotes
      return bScore - aScore
    })
  } else {
    arr.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
  }
  return arr
})

const filteredDiscussions = computed(() => {
  if (!selectedCategory.value) return sortedDiscussions.value
  return sortedDiscussions.value.filter((d) => d.category.name === selectedCategory.value)
})

const totalPages = computed(() =>
  Math.max(1, Math.ceil(filteredDiscussions.value.length / ITEMS_PER_PAGE)),
)

const pagedDiscussions = computed(() => {
  const start = (currentPage.value - 1) * ITEMS_PER_PAGE
  return filteredDiscussions.value.slice(start, start + ITEMS_PER_PAGE)
})

function selectCategory(name: string | null) {
  selectedCategory.value = selectedCategory.value === name ? null : name
}

watch(selectedCategory, () => {
  currentPage.value = 1
})
watch(sortMode, () => {
  currentPage.value = 1
})

// 首次切到市场 tab 时自动加载（避免显示空态后还要手动点刷新）
let marketLoadedOnce = false
watch(activeTab, (tab) => {
  if (tab === 'market' && !marketLoadedOnce) {
    marketLoadedOnce = true
    loadMarket()
  }
})

// ── Display helpers ───────────────────────────────────────────

function getDisplayDescription(d: Discussion): string {
  if (d.description) return d.description
  if (!d.body) return t('settings.workshop.noDesc')
  const plain = d.body
    .replace(/[#*`>\[\]()!|\\]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  const max = 200
  return plain.length <= max ? plain : plain.slice(0, max) + '...'
}

function formatTime(iso: string): string {
  const now = Date.now()
  const then = new Date(iso).getTime()
  const diff = now - then
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return t('settings.workshop.time.justNow')
  if (mins < 60) return t('settings.workshop.time.minutesAgo', { n: mins })
  const hours = Math.floor(mins / 60)
  if (hours < 24) return t('settings.workshop.time.hoursAgo', { n: hours })
  const days = Math.floor(hours / 24)
  if (days < 30) return t('settings.workshop.time.daysAgo', { n: days })
  const months = Math.floor(days / 30)
  if (months < 12) return t('settings.workshop.time.monthsAgo', { n: months })
  return t('settings.workshop.time.yearsAgo', { n: Math.floor(months / 12) })
}

function openDiscussion(url: string) {
  openUrl(url)
}

// ── Load ──────────────────────────────────────────────────────

async function load() {
  loading.value = true
  error.value = null
  try {
    discussions.value = await fetchDiscussions()
    currentPage.value = 1
  } catch (e: unknown) {
    const err = e as { message?: string }
    error.value = typeof e === 'string' ? e : err?.message || t('settings.workshop.loadFailed')
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  load()
  onMarketProgress((p) => {
    progressPercent.value[p.id] = p.percent
    if (p.bytes !== undefined) progressBytes.value[p.id] = p.bytes
    if (p.phase) progressPhase.value[p.id] = p.phase
  }).then((fn) => {
    unlistenProgress = fn
  })
})

onUnmounted(() => {
  unlistenProgress?.()
})
</script>

<style scoped>
.workshop-page {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

/* 背景层：与主菜单同一张背景图，加暗色渐变遮罩保证卡片文字对比度 */
.workshop-page__bg {
  position: absolute;
  inset: -10% -10% 0;
  background-image: url('@/assets/images/background2.png');
  background-size: cover;
  background-position: center;
}

.workshop-page__bg::after {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(
    180deg,
    rgba(0, 0, 0, 0.45),
    rgba(0, 0, 0, 0.3) 40%,
    rgba(0, 0, 0, 0.55)
  );
}

/* 包详情弹窗过渡（Teleport 到 body，scoped 下仍带组件作用域属性） */
.modal-enter-active,
.modal-leave-active {
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
  transform: scale(0.98) translateY(8px);
}
</style>
