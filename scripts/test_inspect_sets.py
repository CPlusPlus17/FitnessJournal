import sys
import json
import garth
from garth.auth_tokens import OAuth1Token, OAuth2Token
from garminconnect import Garmin

def main():
    try:
        with open("secrets/oauth1_token.json") as f:
            oauth1 = json.load(f)
        with open("secrets/oauth2_token.json") as f:
            oauth2 = json.load(f)
    except FileNotFoundError:
        print("Secrets not found")
        return

    try:
        garmin = Garmin("dummy", "dummy")
        garmin.garth.oauth1_token = OAuth1Token(**oauth1)
        garmin.garth.oauth2_token = OAuth2Token(**oauth2)

        # We know 2026_B is a recent strength activity. we need its ID.
        # Let's list activities again to find the ID of "2026_B"
        activities = garmin.get_activities(0, 5)
        target_activity_id = None
        for a in activities:
            if a["activityName"] == "2026_B":
                target_activity_id = a["activityId"]
                break
        
        if not target_activity_id:
            print("Activity 2026_B not found")
            return

        print(f"--- Fetching Sets for {target_activity_id} ---")
        response = garmin.get_activity_exercise_sets(target_activity_id)
        
        # print("Type:", type(response))
        sets = []
        if isinstance(response, dict):
            # likely {'exerciseSets': [...]}
            sets = response.get("exerciseSets", [])
        elif isinstance(response, list):
            sets = response
            
        print(f"Found {len(sets)} sets.")
        
        for i, s in enumerate(sets):
            # Only print unique exercises to avoid spam
            # s['exercises'] is usually a list
            exs = s.get("exercises", [])
            for e in exs:
                # keys might be 'category', 'gnKey', 'name'?
                print(f"Set {i}: {json.dumps(e, indent=2)}")
            if i > 5: break # just sample a few

    except Exception as e:
        print(f"Global Error: {e}")

if __name__ == "__main__":
    main()
