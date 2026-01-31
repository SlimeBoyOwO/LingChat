from datetime import datetime, timedelta

def find_next_time(schedule_times: list[str]) -> str:
        """计算到下一个提醒时间的秒数"""
        now = datetime.now()
        current_time = now.time()
        current_time_str = current_time.strftime("%H:%M")
        
        # 将时间字符串转换为时间对象
        process_schedule_times = [datetime.strptime(time_str, "%H:%M").time() for time_str in schedule_times]
        
        # 找到下一个提醒时间
        next_time = None
        for time_obj in sorted(process_schedule_times):
            if time_obj > current_time:
                next_time = time_obj
                break
        
        # 如果没有找到今天的时间，就用明天第一个时间
        if next_time is None and schedule_times:
            next_time = schedule_times[0]
            # 计算到明天这个时间的秒数
            tomorrow = now + timedelta(days=1)
            next_datetime = datetime.combine(tomorrow.date(), next_time)
        else:
            next_datetime = datetime.combine(now.date(), next_time)

        ans = next_datetime.strftime("%H:%M")
        return ans



def calculate_time_to_next_reminder(schedule_times: list[str]) -> float:
        schedule_times.sort()

        """计算到下一个提醒时间的秒数"""
        now = datetime.now()
        current_time = now.time()
        current_time_str = current_time.strftime("%H:%M")
            
        # 将时间字符串转换为时间对象
        process_schedule_times = [datetime.strptime(time_str, "%H:%M").time() for time_str in schedule_times]
            
        # 找到下一个提醒时间
        next_time = None
        for time_obj in sorted(process_schedule_times):
            if time_obj > current_time:
                next_time = time_obj
                break
            
        # 如果没有找到今天的时间，就用明天第一个时间
        if next_time is None and schedule_times:
            next_time = schedule_times[0]
            # 计算到明天这个时间的秒数
            tomorrow = now + timedelta(days=1)
            next_datetime = datetime.combine(tomorrow.date(), next_time)
        else:
            next_datetime = datetime.combine(now.date(), next_time)

        time_difference = next_datetime - now
        return max(0, time_difference.total_seconds())

schedule_times = ["02:00","18:00","21:00"]

seconds = calculate_time_to_next_reminder(schedule_times)

next_time = find_next_time(schedule_times)

print(seconds)
print(next_time)