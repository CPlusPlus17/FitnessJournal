import os
import sys
import json
from garminconnect import Garmin
from datetime import date

import garth
from garth.auth_tokens import OAuth1Token, OAuth2Token

def main():
    # Load tokens from secrets
    try:
        with open("secrets/oauth1_token.json") as f:
            oauth1 = json.load(f)
        with open("secrets/oauth2_token.json") as f:
            oauth2 = json.load(f)
    except FileNotFoundError:
        sys.stderr.write("Token files not found in secrets/.\n")
        sys.exit(1)

    try:
        # Initialize Garmin client
        garmin = Garmin("dummy", "dummy")
        
        # Debug: Print loaded data keys
        # sys.stderr.write(f"DEBUG: Loaded OAuth1 keys: {list(oauth1.keys())}\n")

        # Convert simple dicts to garth Token objects if needed
        # OAuth1Token/OAuth2Token expect fields matching the JSON directly:
        # oauth_token, oauth_token_secret, etc.
        
        try:
            o1 = OAuth1Token(**oauth1)
            o2 = OAuth2Token(**oauth2)
            
            # sys.stderr.write(f"DEBUG: Created OAuth1Token: {o1}\n")
            
            garmin.garth.oauth1_token = o1
            garmin.garth.oauth2_token = o2
            
            # sys.stderr.write(f"DEBUG: Set garmin.garth.oauth1_token: {garmin.garth.oauth1_token}\n")
            
        except Exception as e:
            sys.stderr.write(f"Failed to create/set Token objects: {e}\n")
            # fallback
            # garmin.garth.oauth1_token = oauth1
        
        # Determine username from token or set dummy if needed for internal logic
        # garmin.username is set by __init__
        
        
        # Skip login() because it forces a check that might be failing on dummy creds
        # forcing it to try re-auth.
        # garmin.login()
        # sys.stderr.write("Skipping login(), attempting API call with injected tokens...\n")

        # Get activities (last 5 to cover recent history)
        # We need the activityId to fetch details (sets/reps)
        try:
             activities = garmin.get_activities(0, 20) # 0 start, 20 limit for better history
        except Exception as e:
             sys.stderr.write(f"Error fetching activities: {e}\n")
             activities = []

        # Enrich strength activities with set details
        for a in activities:
             if a.get("activityType", {}).get("typeKey") == "strength_training":
                 try:
                     activity_id = a["activityId"]
                     # sets = garmin.get_activity_exercise_sets(activity_id) # This gives sets
                     # Actually we need to be careful, get_activity_exercise_sets returns a list.
                     # But looking at previous output, "sets" key was populated? 
                     # Ah, the previous script didn't explicitly fetch sets loop?
                     # Wait, looking at lines 48+ of original script... 
                     # It did: "sets = garmin.get_activity_exercise_sets(activity_id)"
                     # Let me keep that logic.
                     pass 
                 except:
                     pass

        # Re-implement the loop because I am replacing the block
        final_activities = []
        for a in activities:
            # Normalize basic fields
            # ... (keeping existing logic simplistically via a copy if possible, or rewriting)
            # Actually, the previous script logic was:
            # fetch 5 activities
            # for each, if strength, fetch sets and add to 'sets' key
            # append to cleaned list
            
            # I will rewrite the loop to be safe
            clean_act = {
                "id": a["activityId"],
                "name": a["activityName"],
                "type": a["activityType"]["typeKey"],
                "startTimeLocal": a["startTimeLocal"],
                "distance": a.get("distance"),
                "duration": a.get("duration"),
                "averageHR": a.get("averageHR"),
                "maxHR": a.get("maxHR"),
                "sets": [] # Default empty
            }
            
            if clean_act["type"] == "strength_training":
                try:
                    sets = garmin.get_activity_exercise_sets(clean_act["id"])
                    clean_act["sets"] = sets # sets is already {'exerciseSets': [...]}
                except Exception as e:
                    sys.stderr.write(f"Error fetching sets for {clean_act['id']}: {e}\n")
            
            final_activities.append(clean_act)

        # Get Plans
        active_plans = []
        try:
            plans_response = garmin.get_training_plans()
            plan_list = plans_response.get("trainingPlanList", [])
            for p in plan_list:
                # statusId 1 = Scheduled/Active
                if p.get("trainingStatus", {}).get("statusId") == 1:
                    active_plans.append({
                        "name": p.get("name"),
                        "endDate": p.get("endDate"),
                        "type": p.get("trainingType", {}).get("typeKey", "unknown"),
                        "description": p.get("description")
                    })
        except Exception as e:
            sys.stderr.write(f"Error fetching plans: {e}\n")

        # Get User Profile & Metrics
        user_profile = {}
        max_metrics = {}
        try:
            profile = garmin.get_user_profile()
            if "userData" in profile:
                ud = profile["userData"]
                user_profile = {
                    "weight": ud.get("weight"),
                    "height": ud.get("height"),
                    "birthDate": ud.get("birthDate"),
                    "vo2MaxRunning": ud.get("vo2MaxRunning")
                }
        except Exception as e:
            sys.stderr.write(f"Error fetching profile: {e}\n")

        # Fallback/Additional Max Metrics
        try:
            today = date.today().isoformat()
            metrics = garmin.get_max_metrics(today)
            if metrics and len(metrics) > 0:
                # usually returns a list
                gen = metrics[0].get("generic", {})
                max_metrics = {
                    "vo2MaxPrecise": gen.get("vo2MaxPreciseValue"),
                    "fitnessAge": gen.get("fitnessAge")
                }
        except Exception as e:
             sys.stderr.write(f"Error fetching max metrics: {e}\n")

        output = {
            "activities": final_activities,
            "plans": active_plans,
            "user_profile": user_profile,
            "max_metrics": max_metrics
        }

        print(json.dumps(output))

    except Exception as e:
        sys.stderr.write(f"Error: {e}\n")
        sys.exit(1)

if __name__ == "__main__":
    main()
