import sys
import json
import csv
import argparse
import os
from garth.auth_tokens import OAuth1Token, OAuth2Token
from garminconnect import Garmin

# Constants mapping
SPORT_TYPE_STRENGTH = {"sportTypeId": 5, "sportTypeKey": "strength_training"}
STEP_TYPE_WARMUP = {"stepTypeId": 1, "stepTypeKey": "warmup"}
STEP_TYPE_COOLDOWN = {"stepTypeId": 2, "stepTypeKey": "cooldown"}
STEP_TYPE_INTERVAL = {"stepTypeId": 3, "stepTypeKey": "interval"}
STEP_TYPE_REST = {"stepTypeId": 5, "stepTypeKey": "rest"}

CONDITION_REPS = {"conditionTypeId": 10, "conditionTypeKey": "reps"}
CONDITION_TIME = {"conditionTypeId": 2, "conditionTypeKey": "time"}
CONDITION_LAP_BUTTON = {"conditionTypeId": 1, "conditionTypeKey": "lap.button"}

TARGET_NO_TARGET = {"workoutTargetTypeId": 1, "workoutTargetTypeKey": "no.target"}

UNIT_KILOGRAM = {"unitId": 2, "unitKey": "kilogram", "factor": 1000.0}

EXERCISE_DB = {}

def load_exercise_db(csv_path="Garmin Exercises Database - Exercises.csv"):
    global EXERCISE_DB
    if not os.path.exists(csv_path):
        print(f"Warning: Exercise DB CSV not found at {csv_path}. Using name as key.")
        return

    try:
        with open(csv_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                # Keys: Name, CATEGORY_GARMIN, NAME_GARMIN
                
                human_name = row.get("Name", "").strip().upper()
                cat_key = row.get("CATEGORY_GARMIN", "").strip()
                ex_key = row.get("NAME_GARMIN", "").strip()
                
                if human_name and cat_key and ex_key:
                    val = (cat_key, ex_key)
                    
                    # 1. Human Name (e.g. "BENCH PRESS")
                    EXERCISE_DB[human_name] = val
                    
                    # 2. Garmin Key (e.g. "BENCH_PRESS")
                    EXERCISE_DB[ex_key] = val
                    
                    # 3. Underscored Human (e.g. "LAT_PULL_DOWN")
                    EXERCISE_DB[human_name.replace(" ", "_")] = val
                    
                    # 4. Spaced Garmin Key (e.g. "BENCH PRESS")
                    EXERCISE_DB[ex_key.replace("_", " ")] = val
                    
                    # 5. Normalized: Remove hyphens, spaces (e.g. "LATPULLDOWN")
                    norm = human_name.replace("-", "").replace(" ", "").replace("_", "")
                    EXERCISE_DB[norm] = val
                    
                    # 6. Normalized Garmin Key
                    norm_key = ex_key.replace("_", "").replace("-", "")
                    EXERCISE_DB[norm_key] = val

    except Exception as e:
        print(f"Error loading Exercise DB: {e}")

MANUAL_OVERRIDES = {
    "BENT_OVER_ROW": ("ROW", "BARBELL_ROW"),
    "TRICEPS_EXTENSION": ("TRICEPS_EXTENSION", "TRICEP_EXTENSION"), # Singular? Or try ("CURL", "LYING_DUMBBELL_TRICEPS_EXTENSION")
    "PULL_UP": ("PULL_UP", "CHIN_UP"), # Fallback to Chin-up as generic Pull-up is elusive
    "PUSH_UP": ("PUSH_UP", "PUSH_UP"), # Should be valid?
    "LUNGE": ("LUNGE", "LUNGE"), # Should be valid?
    "SQUAT": ("SQUAT", "SQUAT"),
    "DEADLIFT": ("DEADLIFT", "DEADLIFT"),
    "BENCH_PRESS": ("BENCH_PRESS", "BENCH_PRESS"),
    "OVERHEAD_PRESS": ("SHOULDER_PRESS", "SHOULDER_PRESS"),
    "SHOULDER_PRESS": ("SHOULDER_PRESS", "SHOULDER_PRESS"),
    "PLANK": ("PLANK", "PLANK"),
    # Add common aliases
    "LAT_PULLDOWN": ("PULL_UP", "CLOSE_GRIP_LAT_PULLDOWN"), # Best guess or BANDED?
    "RUSSIAN_TWIST": ("CORE", "RUSSIAN_TWIST"),
    
    # New Overrides for specific plan
    "DUMBBELL_ROW": ("ROW", "BENT_OVER_ROW_WITH_DUMBELL"), # Note singular B in DUMBELL
    "BICEP_CURL": ("CURL", "STANDING_ALTERNATING_DUMBBELL_CURLS"),
    "LUNGES": ("LUNGE", "ALTERNATING_DUMBBELL_LUNGE"),
    "GOBLET_SQUAT": ("SQUAT", "GOBLET_SQUAT"),
    "FACE_PULL": ("ROW", "FACE_PULL"),
    "LATERAL_RAISE": ("LATERAL_RAISE", "LATERAL_RAISE"),
    "CALF_RAISE": ("CALF_RAISE", "CALF_RAISE")
}

def resolve_exercise(name):
    # Returns (category_key, exercise_key) or (None, None)
    clean = name.strip().upper()
    
    # 0. Manual Overrides (Highest Priority)
    if clean in MANUAL_OVERRIDES:
        return MANUAL_OVERRIDES[clean]
    
    # 1. Exact Match
    if clean in EXERCISE_DB:
        return EXERCISE_DB[clean]
        
    # 2. Normalized Match (remove all non-alpha)
    norm_input = clean.replace("_", "").replace(" ", "").replace("-", "")
    if norm_input in EXERCISE_DB:
        return EXERCISE_DB[norm_input]

    # 3. Fuzzy Contains (e.g. "DUMBBELL PRESS" -> "DUMBBELL BENCH PRESS"?)
    # Be careful. "PRESS" matches "LEG PRESS". Only if input is specific enough?
    # Skipping pure fuzzy for now to avoid bad matches, trusting normalization.
    
    # 4. Fallback: If looks like a valid key (FOO_BAR), assume it is
    if "_" in clean:
        return (clean, clean)
        
    return (None, None)

def login():
    try:
        with open("secrets/oauth1_token.json") as f:
            oauth1 = json.load(f)
        with open("secrets/oauth2_token.json") as f:
            oauth2 = json.load(f)
        
        garmin = Garmin("dummy", "dummy")
        garmin.garth.oauth1_token = OAuth1Token(**oauth1)
        garmin.garth.oauth2_token = OAuth2Token(**oauth2)
        return garmin
    except Exception as e:
        print(f"Login failed: {e}")
        sys.exit(1)

def parse_duration(val):
    if not val: return None
    if isinstance(val, (int, float)): return int(val)
    if isinstance(val, str):
        import re
        nums = re.findall(r'\d+', val)
        if nums: return int(nums[0])
    return None

def parse_weight(val):
    if val is None or val == "": return None
    if isinstance(val, (int, float)): return float(val)
    import re
    nums = re.findall(r'[\d\.]+', str(val))
    if nums:
        return float(nums[0])
    return None

def build_workout_payload(data, robust=False):
    steps_payload = []
    order = 1
    
    for step in data.get("steps", []):
        raw_name = step.get("exercise", "BENCH_PRESS")
        
        # Determine Step Type
        phase = step.get("phase", "interval").lower()
        step_type = STEP_TYPE_INTERVAL
        if phase == "warmup" or phase == "warm_up":
            step_type = STEP_TYPE_WARMUP
        elif phase == "cooldown" or phase == "cool_down" or phase == "stretching":
            step_type = STEP_TYPE_COOLDOWN
            
        cat_key, ex_key = resolve_exercise(raw_name)
        
        if not cat_key:
            # Fallback if DB lookup completely fails
            cat_key = raw_name.upper().replace(" ", "_")
            ex_key = cat_key

        # End Condition Logic
        reps = step.get("reps")
        duration = step.get("time") or step.get("duration")
        end_cond = CONDITION_LAP_BUTTON
        end_val = None
        
        if reps and step_type != STEP_TYPE_WARMUP and step_type != STEP_TYPE_COOLDOWN:
            if isinstance(reps, str) and "AMRAP" in str(reps).upper():
                end_cond = CONDITION_LAP_BUTTON
            else:
                try:
                    end_val = int(reps)
                    end_cond = CONDITION_REPS
                except:
                    end_cond = CONDITION_LAP_BUTTON
        elif duration:
            sec = parse_duration(duration)
            if sec:
                end_cond = CONDITION_TIME
                end_val = sec

        # Weight Logic
        weight_val = parse_weight(step.get("weight"))
        weight_unit = None
        if weight_val is not None:
            weight_unit = UNIT_KILOGRAM

        # Robust Mode
        category = cat_key
        exercise_name = ex_key
        description = step.get("note")
        
        if robust:
            category = None
            exercise_name = None
            note = step.get("note", "")
            description = f"Exercise: {raw_name} ({ex_key}). {note}".strip()
            if weight_val:
                description += f" Target: {weight_val}kg"

        step_dict = {
            "type": "ExecutableStepDTO",
            "stepOrder": order,
            "stepType": step_type,
            "childStepId": None,
            "description": description, 
            "endCondition": end_cond,
            "endConditionValue": end_val,
            "targetType": TARGET_NO_TARGET,
            "category": category, 
            "exerciseName": exercise_name,
        }
        
        if weight_val is not None and not robust:
            step_dict["weightValue"] = weight_val
            step_dict["weightUnit"] = weight_unit

        steps_payload.append(step_dict)
        order += 1
        
        # Rest Step (Only if NOT warmup/cooldown/stretching)
        if step_type == STEP_TYPE_INTERVAL:
            rest_sec = parse_duration(step.get("rest"))
            if rest_sec:
                steps_payload.append({
                    "type": "ExecutableStepDTO",
                    "stepOrder": order,
                    "stepType": STEP_TYPE_REST,
                    "childStepId": None,
                    "endCondition": CONDITION_TIME,
                    "endConditionValue": rest_sec,
                    "targetType": TARGET_NO_TARGET
                })
                order += 1

    payload = {
        "workoutName": data.get("workoutName", "Imported Strength Workout"),
        "description": data.get("description"),
        "sportType": SPORT_TYPE_STRENGTH,
        "workoutSegments": [
            {
                "segmentOrder": 1,
                "sportType": SPORT_TYPE_STRENGTH,
                "workoutSteps": steps_payload
            }
        ]
    }
    return payload

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("input_file", help="Path to JSON workout definition")
    args = parser.parse_args()

    # Load DB
    load_exercise_db()

    try:
        with open(args.input_file) as f:
            data = json.load(f)
    except Exception as e:
        print(f"Error reading input file: {e}")
        sys.exit(1)

    garmin = login()
    
    workouts = data if isinstance(data, list) else [data]
    
    for w in workouts:
        print(f"Creating workout: {w.get('workoutName')}...")
        
        # First attempt: Specific Mapping via CSV
        payload = build_workout_payload(w, robust=False)
        try:
            response = garmin.connectapi("/workout-service/workout", method="POST", json=payload)
            print(f"Success! Workout ID: {response.get('workoutId')}")
        except Exception as e:
            if "400" in str(e):
                print(f"Failed with CSV mapping (400). Retrying with generic fallback...")
                payload = build_workout_payload(w, robust=True)
                try:
                    response = garmin.connectapi("/workout-service/workout", method="POST", json=payload)
                    print(f"Success! (Generic Mode) Workout ID: {response.get('workoutId')}")
                except Exception as e2:
                    print(f"Failed to create workout (even generic): {e2}")
            else:
                print(f"Failed to create workout: {e}")

if __name__ == "__main__":
    main()
