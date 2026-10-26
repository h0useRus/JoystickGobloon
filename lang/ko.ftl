language-name = 한국어

# Navigation and toolbar tooltips
nav-devices = 장치 및 바인딩
nav-hidhide = 장치 숨기기
nav-apps = 애플리케이션
nav-vjoy = 가상 조이스틱
nav-viewer = 입력 뷰어
nav-calibration = 보정
nav-settings = 설정
action-open = Joystick Gremlin 프로필 가져오기
action-save = 저장
label-mode = 모드

# Settings
settings-appearance = 모양
settings-general = 일반
settings-about = 정보

settings-lighting = 조명
settings-leds-hint = 조이스틱의 LED를 바꾼 프로필이 비활성화되면 이 색상이 설정됩니다. 조이스틱은 다시 연결할 때까지 바뀐 색상을 유지하므로 자체 LED 설정으로 되돌릴 수 없습니다.
settings-leds-none = LED를 제어할 수 있는 조이스틱이 연결되어 있지 않습니다.
settings-leds-add = LED 추가
settings-language = 언어
settings-reopen-last = 시작할 때 마지막 프로필 다시 열기
theme-light = 밝게
theme-dark = 어둡게
theme-system = 시스템
theme-system-tip = 시스템의 밝은/어두운 설정 따르기
about-text = 장치 숨기기, 가상 조이스틱, 입력 매핑을 하나의 앱에서.

# Empty states
no-profile = 불러온 프로필 없음
device-linked = 같은 이름의 프로필 장치 바인딩입니다. “컨트롤 매핑”으로 연결을 저장하세요.
device-disconnected = 연결되지 않음
no-devices-attached = 게임 컨트롤러를 찾을 수 없음

# Status bar
status-ready = 준비
status-loaded = { $file } 불러옴
status-saved = { $file } 저장됨
import-error = 프로필을 열 수 없습니다: { $error }
save-error = 프로필을 저장할 수 없습니다: { $error }
file-filter-gremlin = Joystick Gremlin 프로필

# Import report
alert-error = 문제가 발생했습니다
alert-import = { $file }의 일부 동작이 작동하지 않습니다
alert-open-log = 로그 열기
ok = 확인
hidhide-whitelist-error = Joystick Gobloon을 HidHide 허용 목록에 추가할 수 없습니다: { $error }
warn-not-executable = “{ $kind }” × { $count }: 프로필에 유지되지만 아직 실행되지 않음
warn-scripts = 프로필에 Python 플러그인이 있습니다: 유지되지만 실행되지 않습니다

# Map controls (device substitution)
subst-title = 컨트롤 매핑
subst-auto = 자동
subst-auto-tip = 나머지 장치를 이름이 비슷한 연결된 장치에 매핑합니다. 불확실한 것은 직접 선택하도록 남겨 둡니다
subst-banner = 프로필의 장치 ID가 이 PC와 일치하지 않음: { $count }
access-banner = Joystick Gobloon이 게임 컨트롤러를 읽고 가상 장치를 만들려면 권한이 필요합니다.
access-grant = 접근 허용
access-granted = 장치 접근이 허용되었습니다.
access-error = 장치 접근을 허용할 수 없습니다: { $error }
subst-keep = 그대로 두기
subst-apply = 적용
status-substituted = 컨트롤이 매핑되었습니다. 변경 내용을 유지하려면 프로필을 저장하세요.

# vJoy
vjoy-missing = vJoy가 설치되지 않았거나 불러올 수 없습니다.
vjoy-no-access = vJoy 장치를 만들 권한이 없습니다.
vjoy-enabled = 드라이버 사용
vjoy-disabled = 드라이버 사용 안 함
vjoy-no-devices = 아직 구성된 vJoy 장치가 없습니다.
vjoy-axes = 축
vjoy-buttons = 버튼
vjoy-hats = 햇 스위치
vjoy-test-hint = 출력 테스트: 아래 컨트롤을 사용하려면 장치를 잠그세요.
vjoy-test-pending = 변경 내용을 적용해야 테스트할 수 있습니다.
vjoy-lock = 출력을 테스트하기 위해 이 장치를 독점 제어
vjoy-unlock = 장치 해제
vjoy-free = 사용 가능
vjoy-own = Joystick Gobloon이 제어 중
vjoy-busy = 다른 프로그램 또는 다른 Joystick Gobloon 인스턴스가 사용 중
vjoy-unknown = 알 수 없는 상태
vjoy-error = 장치를 가져올 수 없습니다: { $error }

# Device hiding
hidhide-missing = HidHide가 설치되지 않았습니다.
hidhide-on = 장치 숨기기 켜짐
hidhide-off = 장치 숨기기 꺼짐
hidhide-hidden = 숨김: 허용 목록의 애플리케이션만 볼 수 있음
hidhide-visible = 모든 애플리케이션에 표시
hidhide-filter-disconnected = 연결되지 않은 장치 숨기기
hidhide-gaming-only = 게임 컨트롤러만
hidhide-enable = 장치 숨기기 사용
hidhide-confirm-on-title = 장치 숨기기를 켤까요?
hidhide-confirm-on-text = 숨긴 장치는 화이트리스트에 있는 프로그램만 볼 수 있습니다.
hidhide-confirm-off-title = 장치 숨기기를 끌까요?
hidhide-confirm-off-text = 모든 프로그램이 숨긴 장치를 포함해 모든 장치를 다시 봅니다.
hidhide-confirm-hint = 눈 아이콘을 더블클릭하면 이 질문 없이 전환됩니다.
action-turn-on = 켜기
action-turn-off = 끄기
hidhide-hint = 눈 아이콘을 클릭하면 다른 프로그램에서 장치를 숨깁니다(감은 눈). 변경 내용을 적용하려면 장치를 다시 연결하세요.
hidhide-exposed = 장치를 직접 읽는 프로그램(hidraw, 예: Steam, Wine/Proton)에는 여전히 보입니다. 이 시스템에서는 숨기기로 막을 수 없습니다. 해당 프로그램에서 이 장치의 hidraw를 끄지 않으면 게임에서 두 번 보일 수 있습니다.
hidhide-no-devices = 표시할 장치가 없습니다.
action-refresh = 새로 고침
settings-see-hidden = 숨긴 장치 보기
settings-see-hidden-tip = 시작할 때 이 프로그램이 숨긴 장치를 계속 볼 수 있는 프로그램에 자신을 추가하여 프로필이 계속 작동합니다.
settings-group-startup = 시작
settings-group-devices = 장치
settings-group-editor = 편집기
settings-focus-input = 사용 중인 입력 따라가기
settings-focus-input-tip = 축, 버튼 또는 햇을 사용하면 장치 및 바인딩 페이지가 해당 장치 탭을 선택하고 그 입력으로 스크롤합니다.
settings-focus-skip-modifiers = 모디파이어로 이동하지 않기
settings-focus-skip-modifiers-tip = 모디파이어 버튼이나 키를 눌러도 목록이 그쪽으로 스크롤되지 않아, 설정 중인 입력이 계속 보입니다.
settings-input-events = 변경 시 컨트롤러 읽기
settings-input-events-tip = 지연과 CPU 사용량 감소: 컨트롤러가 변경을 알릴 때 읽으며, 짧은 버튼 입력도 놓치지 않습니다. 끄면 모든 컨트롤러를 폴링 간격마다 읽습니다.
settings-poll-interval = 폴링 간격
settings-poll-interval-tip = 컨트롤러를 얼마나 자주 읽을지: "변경 시 컨트롤러 읽기"가 꺼져 있으면 모든 컨트롤러, 켜져 있으면 변경을 알릴 수 없는 컨트롤러만. 짧을수록 지연이 줄고 CPU 사용량이 약간 늘어납니다.
settings-poll-interval-unused = 지금은 사용되지 않음: 연결된 모든 컨트롤러가 변경을 알리므로 폴링하는 컨트롤러가 없습니다. "변경 시 컨트롤러 읽기"가 꺼져 있거나 변경을 알릴 수 없는 컨트롤러가 있을 때 적용됩니다.
settings-monotonic-curves = 곡선은 증가 또는 감소만
settings-monotonic-curves-tip = 응답 곡선 편집기: 곡선이 되돌아가려는 지점에서 점이나 핸들이 멈추므로 0을 한 번만 지납니다. 이미 되돌아가는 곡선은 자유롭게 편집됩니다.
settings-curve-right-click-invert = 오른쪽 클릭으로 곡선 반전
settings-curve-right-click-invert-tip = 응답 곡선 편집기: 빈 곳을 오른쪽 클릭하면 곡선이 반전됩니다. 반전 버튼은 항상 작동합니다.
settings-warn-gremlin = 저장 시 프로필 형식이 바뀌면 경고
settings-warn-gremlin-tip = 모디파이어나 레이어가 있는 프로필은 Joystick Gremlin이 열 수 없는 형식으로 저장됩니다.
gremlin-format-title = Joystick Gremlin에서 열 수 없습니다
gremlin-format-text = { $file }에는 Joystick Gobloon에만 있는 기능(모디파이어와 레이어)이 사용됩니다. 저장하면 Joystick Gremlin에서 더 이상 열 수 없습니다.
gremlin-format-dont-ask = 다시 묻지 않기
status-whitelisted = Joystick Gobloon을 HidHide 허용 목록에 추가했습니다.

# Applications
apps-add = 애플리케이션 추가
apps-empty = 아직 애플리케이션이 없습니다. 게임이나 프로그램을 추가해 설정하세요.
apps-missing = 파일을 찾을 수 없음
apps-missing-text = 프로그램이 더 이상 없습니다. 목록에서 제거할까요?
apps-sees = 숨겨진 장치를 볼 수 있음
apps-blind = 숨겨진 장치를 볼 수 없음
apps-inverse = 애플리케이션 목록 반전
apps-inverse-tip = 목록의 프로그램은 숨겨진 장치를 보지 못하고, 다른 모든 프로그램은 봅니다
apps-remove = 목록에서 제거

# Engine
action-activate = 프로필 활성화
action-deactivate = 프로필 중지
title-active = 활성
title-inactive = 비활성
status-active = 프로필 활성화됨
status-stopped = 프로필 중지됨
activate-error = 프로필을 활성화할 수 없습니다: { $error }

# Editing
edit-add = 동작 추가
edit-remove = 제거
edit-up = 위로 이동
edit-down = 아래로 이동
edit-not-editable = 이 동작은 프로필에 유지되지만 아직 편집할 수 없습니다.
edit-on-press = 누를 때
edit-on-release = 뗄 때
edit-on-both = 누를 때와 뗄 때
edit-inverted = 반전됨
edit-threshold = 길게 누르기 기준 시간
edit-short = 짧게 누르기
edit-long = 길게 누르기
edit-deadzone = 데드존
edit-invert = 반전
edit-inherited = “{ $mode }” 모드에서 상속됨
edit-unbind = 이 입력의 모든 동작 제거
edit-override = 이 모드에서 재정의
input-axis-kind = 축
input-button-kind = 버튼
input-hat-kind = 햇 스위치
kind-map-to-vjoy = vJoy에 매핑
kind-response-curve = 응답 곡선
kind-change-mode = 모드 전환
kind-tempo = 짧게 / 길게 누르기
kind-description = 설명

# Files and modes
action-new = 새 프로필
action-save-as = 다른 이름으로 저장…
status-new = 새 프로필
unsaved-title = 저장하지 않은 변경 내용
unsaved-text = { $file }의 변경 내용을 저장할까요?
unsaved-save = 저장
unsaved-discard = 저장 안 함
cancel = 취소
modes-edit = 모드 편집
modifiers-edit = 모디파이어
modifiers-none = 아직 모디파이어가 없습니다. 모디파이어는 누르고 있는 동안 다른 입력을 다른 동작 레이어로 전환하는 버튼입니다.
modifiers-add = 모디파이어 추가
modifiers-press = 컨트롤러의 버튼을 누르세요…
modifiers-assign = 클릭한 다음 이 모디파이어가 사용할 버튼을 누르세요
modifiers-remove = 모디파이어와 그 레이어의 동작 제거
modifier-input = 모디파이어 { $name }: 누르고 있는 동안 다른 입력이 이 레이어를 사용합니다. 자체 동작은 실행하지 않습니다
layer-base = 기본
layer-set = 이 레이어에 동작이 있습니다
layer-empty = 이 레이어에 동작이 없습니다
layer-additive = 기본에 추가
layer-additive-tip = 켜짐: 이 동작이 기본 레이어와 함께 실행됩니다. 꺼짐: 모디파이어를 누르고 있는 동안 기본 레이어를 대체합니다.
layer-keep-value = 마지막 값 유지
layer-keep-value-tip = 켜짐: 이 레이어가 멈추면 축이 마지막 값을 유지합니다. 꺼짐: 축이 기본 위치로 돌아갑니다.
layers-tip = 레이어: { $layers }
modes-title = 모드
modes-name = 이름
modes-parent = 상속 대상
modes-remove = 모드와 그 바인딩 제거
modes-new = 새 모드
modes-add = 모드 추가

# Undo
action-undo = 실행 취소 (Ctrl+Z)
action-redo = 다시 실행 (Ctrl+Y)
status-undone = 실행 취소됨
status-redone = 다시 실행됨
edit-curve-help = 점이나 핸들을 끌어 이동 · 더블클릭으로 점 추가 · 점을 오른쪽 클릭해 제거
edit-curve-help-invert = 빈 곳 오른쪽 클릭으로 반전

# vJoy mapping
vjoy-device-n = vJoy #{ $n }
vjoy-axis-1 = X 축
vjoy-axis-2 = Y 축
vjoy-axis-3 = Z 축
vjoy-axis-4 = X 회전
vjoy-axis-5 = Y 회전
vjoy-axis-6 = Z 회전
vjoy-axis-7 = 슬라이더
vjoy-axis-8 = 다이얼
vjoy-button-n = 버튼 { $n }
vjoy-hat-n = 햇 스위치 { $n }
vjoy-absolute = 절대
vjoy-relative = 상대
vjoy-speed = 속도

# Mouse
kind-map-to-mouse = 마우스에 매핑
mouse-mode-button = 마우스 버튼
mouse-mode-motion = 커서 이동
mouse-left = 왼쪽 버튼
mouse-right = 오른쪽 버튼
mouse-middle = 가운데 버튼
mouse-forward = 앞으로 버튼
mouse-back = 뒤로 버튼
mouse-wheel-up = 휠 위로
mouse-wheel-down = 휠 아래로
mouse-direction = 방향
mouse-min-speed = 최저 속도
mouse-max-speed = 최고 속도
mouse-time-to-max = 최고 속도까지 걸리는 시간

# Toggle, split, pause, run
kind-smart-toggle = 스마트 토글
kind-split-axis = 축 분할
kind-pause-resume = 일시 중지 / 재개
kind-run-command = 명령 실행
edit-delay = 모멘터리 동작의 누름 유지 시간
edit-split = 분할 지점
edit-lower = 아래쪽
edit-upper = 위쪽
pause-op-pause = 일시 중지
pause-op-resume = 재개
pause-op-toggle = 전환
run-executable = 프로그램
run-arguments = 인수
run-browse = 프로그램 선택
status-paused = 프로필 일시 중지됨
status-latency = 컨트롤러를 읽은 뒤 가상 출력까지의 지연, 최근 1초: 평균 { $average } ms, 최대 { $max } ms. 컨트롤러 자체의 지연은 포함되지 않습니다.

# Double tap, chain
kind-double-tap = 더블 탭
kind-chain = 체인
edit-tap-threshold = 두 번째 탭까지의 시간
edit-tap-exclusive = 배타적: 한 번 탭은 더블 탭이 아닌지 확인할 때까지 기다림
edit-tap-combined = 결합: 더블 탭도 한 번 탭 동작을 실행
edit-single = 한 번 탭
edit-double = 더블 탭
edit-chain-timeout = 이 시간 동안 입력이 없으면 첫 단계부터 다시 시작 (0 = 안 함)
edit-chain-step = { $n }단계

# Hat as buttons
kind-hat-buttons = 햇 스위치를 버튼으로
edit-hat-four = 4방향
edit-hat-eight = 8방향 (대각선 포함)
hat-north = 북
hat-north-east = 북동
hat-east = 동
hat-south-east = 남동
hat-south = 남
hat-south-west = 남서
hat-west = 서
hat-north-west = 북서
hat-center = 중앙

# Merge axis
kind-merge-axis = 축 병합
edit-reuse = 이 기존 동작을 여기에도 추가 (Gremlin처럼 두 축에 공유로 바인딩)
edit-merge-name = 이름
edit-merge-no-device = (장치 없음)
edit-merged = 병합된 축
merge-average = 평균
merge-minimum = 최솟값
merge-maximum = 최댓값
merge-sum = 합계
merge-bidirectional = 양방향 (축 2 빼기 축 1)
merge-maximum-deflection = 최대 편향
merge-closest-to-center = 중앙에 가장 가까운 값

# Axis delta
kind-axis-delta = 축 변화량
edit-delta-threshold = 펄스를 발생시키는 축 변화량
edit-delta-positive = 축 증가
edit-delta-negative = 축 감소

# Dual axis deadzone
kind-dual-axis-deadzone = 이중 축 데드존
edit-dual-inner = 안쪽 데드존 (중앙 주변의 원)
edit-dual-outer = 바깥 한계 (여기서부터 최대 편향)
edit-dual-close = 안쪽과 바깥 데드존이 너무 가깝습니다.
edit-dual-first = 첫 번째 축 출력
edit-dual-second = 두 번째 축 출력

# Existing actions (Gremlin's reference action)
edit-link-existing = 기존 동작 연결
edit-copy-existing = 기존 동작 복사

# Condition
kind-condition = 조건
edit-cond-all = 모든 조건을 만족해야 함
edit-cond-any = 하나의 조건만 만족하면 됨
edit-cond-add = 조건
edit-cond-true = 참이면
edit-cond-false = 아니면
cond-current = 이 입력
cond-joystick = 조이스틱 입력
cond-vjoy = vJoy 출력
cond-keyboard = 누르고 있는 키
cond-range = 축 범위
cond-pressed = 눌림
cond-released = 떼어짐

# Sound, speech, load profile
kind-play-sound = 소리 재생
kind-text-to-speech = 텍스트 음성 변환
kind-load-profile = 프로필 불러오기
file-filter-sound = 소리 파일
edit-volume = 볼륨
edit-file-missing = 파일을 찾을 수 없음
edit-tts-hint = 읽을 텍스트; $current_mode는 현재 모드로 바뀝니다
edit-tts-rate = 속도
edit-tts-pitch = 음높이
edit-tts-back = 모든 항목 뒤에 대기
edit-tts-front = 다음 순서로 대기 (시스템 음성: 모든 항목 뒤와 같음)
edit-tts-interrupt = 읽고 있는 내용 중단
edit-preview = 미리 듣기

kind-light-led = LED 조명
edit-led-device = LED가 있는 조이스틱
edit-led-no-device = 이 조이스틱이 연결되어 있지 않거나 LED를 제어할 수 없습니다
edit-led-id = LED
led-vkb-grip = 그립 상단 RGB
led-vkb-hat = 햇 스위치 옆 작은 빨간색
edit-led-color = 색상

edit-led-color2 = 두 번째 색상
edit-led-blend = 색상
led-blend-color1 = 한 가지 색상
led-blend-color2 = 두 번째 색상만
led-blend-alternate = 색상 번갈아
led-blend-alternate-reverse = 번갈아, 두 번째 색상 먼저
led-blend-mix = 색상 혼합
led-vkb-base-blue = 베이스, 파란색 (오른쪽)
led-vkb-base-red = 베이스, 빨간색 (왼쪽)
edit-led-mode = 점등 방식
led-mode-on = 켜짐
led-mode-slow-blink = 느리게 깜박임
led-mode-fast-blink = 빠르게 깜박임
led-mode-fastest-blink = 매우 빠르게 깜박임
led-mode-off = 꺼짐
edit-led-test = 지금 켜기
edit-led-on-activate = 프로필 또는 모드가 활성화될 때
edit-tts-voice = 음성
edit-tts-default-voice = 기본 음성
edit-tts-voice-missing = 설치되지 않음
kind-map-to-keyboard = 키보드에 매핑

# Key recording
keys-record = 키 녹화: 함께 누르세요. 모두 떼면 녹화가 끝납니다
keys-press = 키를 누르세요…
keys-none = 키 없음
keys-cancel = 녹화 취소
edit-chain-add = 단계 추가
edit-chain-remove = 이 단계와 그 동작 제거
kind-macro = 매크로

# Macro editor
macro-repeat = 반복
macro-repeat-single = 한 번
macro-repeat-count = 횟수
macro-repeat-hold = 누르고 있는 동안
macro-repeat-toggle = 다시 누를 때까지
macro-repeat-count-tip = 반복 횟수
macro-repeat-delay = 반복 사이 대기
macro-add-step = 단계
macro-step-key = 키
macro-step-pause = 대기
macro-step-mouse-button = 마우스 버튼
macro-step-mouse-motion = 마우스 이동
macro-step-vjoy-button = vJoy 버튼
macro-step-vjoy-axis = vJoy 축
macro-step-vjoy-hat = vJoy 햇 스위치
macro-press = 누르기
macro-release = 떼기

# Logical device
logical-device = 논리 장치
logical-device-tip = 이 프로필의 가상 입력: 입력을 여기에 매핑한 다음 스틱처럼 바인딩하세요
logical-add-axis = 축 추가
logical-add-button = 버튼 추가
logical-add-hat = 햇 스위치 추가
logical-remove = 이 입력과 그 바인딩 제거
logical-empty = 아직 논리 입력이 없습니다: 위 버튼으로 추가하세요.
kind-map-to-logical-device = 논리 장치에 매핑
cond-logical = 논리 장치 입력

# Tray and startup
settings-tray = 트레이 아이콘 표시
settings-close-to-tray = 닫을 때 트레이로
settings-minimize-to-tray = 최소화할 때 트레이로
settings-tray-double-click = 두 번 클릭하면 프로필 활성화
settings-tray-title = 트레이 아이콘
settings-autostart = 로그인 시 시작 (최소화)
settings-activate-on-start = 시작할 때 프로필 활성화
tray-show = Joystick Gobloon 표시
tray-hiding = 장치 숨기기
tray-activate = “{ $name }” 활성화
tray-activate-none = 프로필 활성화
tray-quit = 종료
autostart-error = 로그인 시 시작 설정을 바꿀 수 없습니다: { $error }

# About
about-alpha = 알파 빌드
about-created = 만든 사람: { $name }
about-support = 개발 후원
about-support-tip = PayPal로 후원하기
about-source = 소스 코드
about-issues = 문제 신고
about-releases = 릴리스
about-hiding = 장치 숨기기: { $backend }
about-virtual = 가상 조이스틱: { $backend }
about-not-installed = 설치되지 않음
about-gremlin = 프로필은 Joystick Gremlin과 호환됩니다.
about-license = MIT 라이선스에 따른 무료 오픈 소스 소프트웨어입니다.

# Updates
settings-check-updates = 시작할 때 새 버전을 확인하고 설치
update-check = 업데이트 확인
update-checking = 새 버전을 찾는 중…
update-latest = 최신 버전입니다
update-available = { $version } 버전을 사용할 수 있습니다
update-install = 지금 업데이트
update-downloading = { $version } 버전을 다운로드하는 중…
update-error = 업데이트를 확인할 수 없습니다
update-debug = 디버그 빌드는 업데이트되지 않습니다
update-install-error = 업데이트를 설치할 수 없습니다: { $error }

# Input viewer
viewer-history = 시간에 따른 축
viewer-current = 현재 축
viewer-buttons = 버튼과 햇 스위치
viewer-axis = 축 { $n }

# vJoy configuration
vjoy-add = vJoy 장치 추가
vjoy-add-max = vJoy는 최대 16개의 장치를 지원합니다.
vjoy-new-device = 새 vJoy #{ $n }: 아직 만들지 않음
vjoy-enable = vJoy 드라이버 사용
vjoy-disable = vJoy 드라이버 사용 안 함
vjoy-hat-continuous = 연속
vjoy-hat-discrete = 4방향
vjoy-ffb = 포스 피드백
vjoy-ffb-enable = 사용
vjoy-ffb-const = 일정
vjoy-ffb-ramp = 램프
vjoy-ffb-sq = 사각파
vjoy-ffb-sine = 사인파
vjoy-ffb-tr = 삼각파
vjoy-ffb-stup = 상승 톱니파
vjoy-ffb-stdn = 하강 톱니파
vjoy-ffb-spr = 스프링
vjoy-ffb-dm = 댐퍼
vjoy-ffb-inr = 관성
vjoy-ffb-fric = 마찰
vjoy-config-create = 만들기
vjoy-config-apply = 적용
vjoy-config-revert = 되돌리기
vjoy-config-remove = 장치 제거
vjoy-config-admin = vJoy를 변경하려면 Windows에서 관리자 권한을 요청합니다
vjoy-config-locked = vJoy 장치를 변경하려면 프로필을 중지하세요.
vjoy-config-reading = vJoy 구성을 읽는 중...
vjoy-config-label = vJoy #{ $n } 구성
vjoy-config-done = vJoy 구성이 변경되었습니다.
vjoy-config-error = vJoy 구성을 변경할 수 없습니다: { $error }
vjoy-config-reboot = vJoy 구성이 변경되었습니다. 적용하려면 Windows를 다시 시작하세요.
vjoy-remove-title = vJoy #{ $n }을(를) 제거할까요?
vjoy-remove-text = 이 장치를 사용하는 게임과 프로필은 다시 만들 때까지 이 장치를 잃습니다.

# Response curve
curve-type = 곡선 유형
curve-linear = 구간 선형
curve-cubic = 3차 스플라인
curve-monotone = 부드럽게, 오버슈트 없음
curve-bezier = 3차 베지어 스플라인
curve-symmetric = 대칭
curve-symmetric-tip = 편집하는 동안 곡선을 중앙 기준으로 대칭
curve-point = 점
curve-handle = 핸들

# Keyboard device
keyboard-device = 키보드
keyboard-tip = 키보드 키를 입력으로: 조이스틱 버튼처럼 키에 동작을 바인딩
keyboard-add = 키 바인딩
keyboard-empty = 아직 바인딩된 키가 없습니다: 위 버튼으로 녹화하세요.
edit-binding-description = 이 입력의 설명
edit-unbound = 아직 동작이 없습니다: 하나 추가하세요.
viewer-detach = 별도 창에서 열기
viewer-attach = 기본 창으로 돌아가기
viewer-in-window = 입력 뷰어가 별도 창에 열려 있습니다.

# Calibration
calibration-device = 보정할 장치
calibration-raw = 원시 값
calibration-with-center = 중앙 포함
calibration-reset = 초기화
calibration-save = 이 보정 저장
calibration-center = 중앙 보정
calibration-center-tip = 스틱을 놓아 두고 중앙 주변에서 살짝 움직인 다음 다시 클릭하세요
calibration-extrema = 끝점 보정
calibration-extrema-tip = 축을 양 끝까지 움직인 다음 다시 클릭하세요
calibration-in-window = 보정이 별도 창에 열려 있습니다.

# Device info
device-info = 장치 정보

device-leds = LED 제어 가능 (LED 조명 동작)
device-info-name = 장치 이름
device-info-axes = 축
device-info-buttons = 버튼
device-info-hats = 햇 스위치
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = 조이스틱 ID
device-info-guid = 장치 GUID
device-info-copy = 클립보드에 복사

# Virtual gamepads
kind-map-to-gamepad = 게임패드로 매핑
gamepad-n = 게임패드 { $n }
gamepad-tip = Xbox 스타일 가상 게임패드: 게임, Steam, Proton이 Xbox 360 컨트롤러로 인식합니다.
gamepad-add = 가상 게임패드 추가
gamepad-add-max = 게임패드는 최대 { $n }개입니다.
gamepad-add-n = 게임패드 { $n } 추가
gamepad-delete-n = 게임패드 { $n } 삭제
gamepad-missing = 게임패드 { $n }이(가) 아직 없습니다: 가상 조이스틱 페이지에서 추가하세요.
pad-a = A
pad-b = B
pad-x = X
pad-y = Y
pad-lb = LB
pad-rb = RB
pad-back = Back
pad-start = Start
pad-guide = Guide
pad-ls = 왼쪽 스틱 누르기
pad-rs = 오른쪽 스틱 누르기
pad-dpad-up = 방향 패드 위
pad-dpad-down = 방향 패드 아래
pad-dpad-left = 방향 패드 왼쪽
pad-dpad-right = 방향 패드 오른쪽
pad-left-x = 왼쪽 스틱 X
pad-left-y = 왼쪽 스틱 Y
pad-right-x = 오른쪽 스틱 X
pad-right-y = 오른쪽 스틱 Y
pad-lt = LT (왼쪽 트리거)
pad-rt = RT (오른쪽 트리거)
pad-dpad = 방향 패드
