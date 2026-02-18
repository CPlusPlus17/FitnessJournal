import sys
import json
import argparse
from garth.auth_tokens import OAuth1Token, OAuth2Token
from garminconnect import Garmin

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

def main():
    garmin = login()
    
    # Fetch list of workouts
    print("Fetching workouts...")
    try:
        workouts = garmin.connectapi("/workout-service/workouts")
    except Exception as e:
        print(f"Failed to fetch workouts: {e}")
        sys.exit(1)

    # Filter for "Strength *"
    to_delete = []
    for w in workouts:
        name = w.get("workoutName", "")
        if name.startswith("Strength ") or name.startswith("Strength A") or name.startswith("Strength B") or name.startswith("Strength C"):
            to_delete.append(w)
            
    print(f"Found {len(to_delete)} workouts to delete:")
    for w in to_delete:
        print(f" - {w['workoutId']}: {w['workoutName']}")

    if not to_delete:
        print("No workouts found matching 'Strength *'.")
        return

    # Delete
    print("\nDeleting...")
    for w in to_delete:
        wid = w['workoutId']
        try:
            garmin.connectapi(f"/workout-service/workout/{wid}", method="DELETE")
            print(f"Deleted {wid} ({w['workoutName']})")
        except Exception as e:
             print(f"Failed to delete {wid}: {e}")

if __name__ == "__main__":
    main()
