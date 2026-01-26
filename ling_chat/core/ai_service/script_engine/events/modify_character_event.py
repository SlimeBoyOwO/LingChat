from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.emotion.classifier import emotion_classifier
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory


class ModifyCharacterEvent(BaseEvent):
    """处理角色修改事件"""

    async def execute(self):
        character:str = self.event_data.get('character', '')
        emotion = self.event_data.get('emotion', '')
        duration = self.event_data.get('duration', 1.0)
        action = self.event_data.get('action','')
        perceive = self.event_data.get('perceive', '')

        # 我觉得这个事件没必要加到历史事件.jpg
        # self.game_context.dialogue.append({
        #     'character': character,
        #     'text': text,
        # })

        role = ScriptFunction.get_role(self.game_status, self.script_status, character)
        character_id = role.role_id

        predictedEmotion = ""

        if emotion:
            if emotion != "AI思考" and emotion != "正常":
                predicted = emotion_classifier.predict(emotion)
                prediction_result = {
                    "label": predicted["label"],
                    "confidence": predicted["confidence"]
                }
                predictedEmotion = prediction_result['label']
            else:
                predictedEmotion = emotion

        # 构建参数字典，只包含非空的参数
        params = {
            'characterId': character_id,
            'duration': duration
        }

        if predictedEmotion:
            params['emotion'] = predictedEmotion

        if action:
            if action == "show_character":
                self.game_status.present_roles.add(role)
            elif action == "hide_character":
                self.game_status.present_roles.remove(role)

            params['action'] = action
        
        if perceive != '':
            if perceive:
                self.game_status.present_roles.add(role)
            else:
                self.game_status.present_roles.remove(role)

            # params['perceive'] = perceive

        event_response = ResponseFactory.create_modify_character(**params)
        await message_broker.publish(self.client_id, event_response.model_dump())

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'modify_character'
